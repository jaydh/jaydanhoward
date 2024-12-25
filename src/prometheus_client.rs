use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PrometheusData {
    // Define the structure based on the Prometheus API response
    pub status: String,
    pub data: PrometheusResult,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusResult {
    pub result_type: String,
    pub result: Vec<PrometheusMetric>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrometheusMetric {
    metric: std::collections::HashMap<String, String>,
    value: (f64, String),
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub async fn query_prometheus(query: &str) -> Result<PrometheusData, anyhow::Error> {
    use reqwest::Client;
    let client = Client::new();
    match std::env::var("PROMETHEUS_URL") {
        Ok(base_url) => {
            let url = format!("{}/api/v1/query?query={}", base_url, query);

            let response = client
                .get(&url)
                .send()
                .await?
                .json::<PrometheusData>()
                .await?;
            Ok(response)
        }
        Err(_) => Err(anyhow!("Prometheus URL not defined")),
    }
}
