use actix_web::{web, HttpResponse};

pub struct WorldMapSvg(pub String);

const GEOJSON_URL: &str = concat!(
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector",
    "/master/geojson/ne_110m_land.geojson"
);

// Equirectangular: x = lon + 180, y = 90 - lat  (viewBox 0 0 360 180)
fn project(lon: f64, lat: f64) -> (f64, f64) {
    (lon + 180.0, 90.0 - lat)
}

fn ring_to_path(coords: &[[f64; 2]]) -> String {
    let mut s = String::new();
    for (i, c) in coords.iter().enumerate() {
        let (x, y) = project(c[0], c[1]);
        if i == 0 {
            s.push_str(&format!("M{x:.2},{y:.2}"));
        } else {
            s.push_str(&format!("L{x:.2},{y:.2}"));
        }
    }
    s.push('Z');
    s
}

fn geometry_paths(geom: &serde_json::Value) -> Vec<String> {
    let mut paths = Vec::new();
    match geom["type"].as_str() {
        Some("Polygon") => {
            if let Some(rings) = geom["coordinates"][0].as_array() {
                let coords: Vec<[f64; 2]> = rings
                    .iter()
                    .filter_map(|c| Some([c[0].as_f64()?, c[1].as_f64()?]))
                    .collect();
                paths.push(ring_to_path(&coords));
            }
        }
        Some("MultiPolygon") => {
            if let Some(polys) = geom["coordinates"].as_array() {
                for poly in polys {
                    if let Some(rings) = poly[0].as_array() {
                        let coords: Vec<[f64; 2]> = rings
                            .iter()
                            .filter_map(|c| Some([c[0].as_f64()?, c[1].as_f64()?]))
                            .collect();
                        paths.push(ring_to_path(&coords));
                    }
                }
            }
        }
        _ => {}
    }
    paths
}

pub async fn fetch_world_map_svg(client: &reqwest::Client) -> String {
    let geojson = match client.get(GEOJSON_URL).send().await {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(j) => j,
            Err(_) => return fallback_svg(),
        },
        Err(_) => return fallback_svg(),
    };

    let mut all_paths = String::new();
    if let Some(features) = geojson["features"].as_array() {
        for feature in features {
            for path in geometry_paths(&feature["geometry"]) {
                all_paths.push_str(&path);
                all_paths.push(' ');
            }
        }
    }

    if all_paths.is_empty() {
        return fallback_svg();
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 180">
  <rect width="360" height="180" fill="#0f172a"/>
  <path fill="#1e293b" stroke="#334155" stroke-width="0.3" stroke-linejoin="round" d="{all_paths}"/>
  <line x1="0" y1="90" x2="360" y2="90" stroke="#1e3a5f" stroke-width="0.3" opacity="0.6"/>
  <line x1="180" y1="0" x2="180" y2="180" stroke="#1e3a5f" stroke-width="0.3" opacity="0.4"/>
</svg>"##
    )
}

fn fallback_svg() -> String {
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 180">
  <rect width="360" height="180" fill="#0f172a"/>
  <line x1="0" y1="90" x2="360" y2="90" stroke="#1e3a5f" stroke-width="0.3" opacity="0.6"/>
  <line x1="180" y1="0" x2="180" y2="180" stroke="#1e3a5f" stroke-width="0.3" opacity="0.4"/>
</svg>"##
    .to_string()
}

pub async fn world_map(svg: web::Data<WorldMapSvg>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/svg+xml")
        .append_header(("Cache-Control", "public, max-age=86400"))
        .body(svg.get_ref().0.clone())
}
