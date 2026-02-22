use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::future::{ready, Future, Ready};
use std::net::IpAddr;
use std::pin::Pin;

#[cfg(feature = "ssr")]
use sqlx::PgPool;

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct VisitorLogger {
    pool: Option<PgPool>,
    http_client: reqwest::Client,
}

#[cfg(feature = "ssr")]
impl VisitorLogger {
    pub fn new(pool: Option<PgPool>, http_client: reqwest::Client) -> Self {
        Self { pool, http_client }
    }
}

#[cfg(feature = "ssr")]
impl<S, B> Transform<S, ServiceRequest> for VisitorLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = VisitorLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(VisitorLoggerMiddleware {
            service,
            pool: self.pool.clone(),
            http_client: self.http_client.clone(),
        }))
    }
}

#[cfg(feature = "ssr")]
pub struct VisitorLoggerMiddleware<S> {
    service: S,
    pool: Option<PgPool>,
    http_client: reqwest::Client,
}

#[cfg(feature = "ssr")]
impl<S, B> Service<ServiceRequest> for VisitorLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let pool = match &self.pool {
            Some(p) => p.clone(),
            None => {
                let fut = self.service.call(req);
                return Box::pin(fut);
            }
        };

        let path = req.path().to_string();

        if !should_log(&path) {
            let fut = self.service.call(req);
            return Box::pin(fut);
        }

        let ip = {
            let raw = req
                .connection_info()
                .realip_remote_addr()
                .unwrap_or("unknown")
                .to_string();
            // Strip port if present: "1.2.3.4:port" or "[::1]:port" → bare IP
            raw.parse::<std::net::SocketAddr>()
                .map(|s| s.ip().to_string())
                .unwrap_or(raw)
        };

        if is_private_ip(&ip) {
            let fut = self.service.call(req);
            return Box::pin(fut);
        }

        let http_client = self.http_client.clone();

        tokio::task::spawn(async move {
            record_visit(pool, http_client, ip, path).await;
        });

        let fut = self.service.call(req);
        Box::pin(fut)
    }
}

fn should_log(path: &str) -> bool {
    if path == "/health_check" || path.starts_with("/jaydanhoward_wasm/") {
        return false;
    }
    if path.contains('.') {
        if let Some(ext) = path.rsplit('.').next() {
            return !matches!(
                ext,
                "wasm"
                    | "js"
                    | "css"
                    | "woff2"
                    | "woff"
                    | "ttf"
                    | "eot"
                    | "otf"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "svg"
                    | "ico"
                    | "webp"
                    | "map"
            );
        }
    }
    true
}

fn is_private_ip(ip_str: &str) -> bool {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => true,
    }
}

#[cfg(feature = "ssr")]
async fn record_visit(pool: PgPool, http_client: reqwest::Client, ip: String, path: String) {
    use crate::db::insert_visit;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GeoResponse {
        status: String,
        country: Option<String>,
        country_code: Option<String>,
        region_name: Option<String>,
        city: Option<String>,
        lat: Option<f64>,
        lon: Option<f64>,
        isp: Option<String>,
    }

    let geo: Option<GeoResponse> = async {
        let resp = http_client
            .get(format!("http://ip-api.com/json/{ip}?fields=status,country,countryCode,regionName,city,lat,lon,isp"))
            .send()
            .await
            .ok()?;
        resp.json::<GeoResponse>().await.ok()
    }
    .await
    .filter(|g| g.status == "success");

    let (country, country_code, region, city, lat, lon, isp) = match geo {
        Some(g) => (
            g.country,
            g.country_code,
            g.region_name,
            g.city,
            g.lat,
            g.lon,
            g.isp,
        ),
        None => (None, None, None, None, None, None, None),
    };

    let _ = insert_visit(
        &pool,
        &ip,
        &path,
        country.as_deref(),
        country_code.as_deref(),
        region.as_deref(),
        city.as_deref(),
        lat,
        lon,
        isp.as_deref(),
    )
    .await;
}
