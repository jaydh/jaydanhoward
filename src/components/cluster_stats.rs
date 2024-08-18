#![allow(clippy::all)]
use crate::components::source_anchor::SourceAnchor;
use anyhow::{Context, Result};
use leptos::*;
use reqwest::{Client, Url};

#[server(ActixExtract, "/api")]
pub async fn actix_extract() -> Result<String, ServerFnError<String>> {
    dbg!("whaaaaa");
    let base_url = std::env::var("PROMETHEUS_URL")
        .context("Prometheus url not configured")
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let mut url = Url::parse(&base_url)
        .context("Failed to parse URL")
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;
    url.query_pairs_mut().append_pair(
        "query",
        r#"sum(rate(container_cpu_usage_seconds_total[5m])) by (cluster)"#,
    );

    let response = reqwest::get(url)
        .await
        .context("Failed to make the request")
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let body = response
        .text()
        .await
        .context("Failed to read response body")
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;
    dbg!("Body: {:?}", body);

    Ok("what".into())
}

#[component]
pub fn ClusterStats() -> impl IntoView {
    let once = create_resource(|| (), |_| async move { actix_extract().await });

    view! {
        <SourceAnchor href="#[git]" />
        {move || {
            once.get()
                .map(|string| {
                    view! { <div>string</div> }
                })
        }}
    }
}
