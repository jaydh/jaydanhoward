//! Ported verbatim from src/prometheus_client.rs — plain HTTP client
//! against PROMETHEUS_URL, no special SDK. Confirmed (via research this
//! session) to be a normal env-var-configured URL: unreachable from an
//! external dev laptop (NodePort is LAN-only), but works fine from inside
//! the cluster where the production pod actually runs — this milestone's
//! live verification is deferred to milestone 8's in-cluster staging step.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct PrometheusData {
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
    pub metric: HashMap<String, String>,
    pub value: (f64, String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrometheusRangeData {
    pub status: String,
    pub data: PrometheusRangeResult,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusRangeResult {
    pub result_type: String,
    pub result: Vec<PrometheusRangeMetric>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrometheusRangeMetric {
    pub metric: HashMap<String, String>,
    pub values: Vec<(f64, String)>,
}

pub fn empty_data() -> PrometheusData {
    PrometheusData {
        status: String::new(),
        data: PrometheusResult { result_type: String::new(), result: vec![] },
    }
}

pub async fn query_prometheus(query: &str) -> Result<PrometheusData, anyhow::Error> {
    let base_url = std::env::var("PROMETHEUS_URL").map_err(|_| anyhow::anyhow!("Prometheus URL not defined"))?;
    let response = reqwest::Client::new()
        .get(format!("{base_url}/api/v1/query"))
        .query(&[("query", query)])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .json::<PrometheusData>()
        .await?;
    Ok(response)
}

pub async fn query_prometheus_range(
    query: &str,
    start: i64,
    end: i64,
    step: &str,
) -> Result<PrometheusRangeData, anyhow::Error> {
    let base_url = std::env::var("PROMETHEUS_URL").map_err(|_| anyhow::anyhow!("Prometheus URL not defined"))?;
    let response = reqwest::Client::new()
        .get(format!("{base_url}/api/v1/query_range"))
        .query(&[
            ("query", query),
            ("start", &start.to_string()),
            ("end", &end.to_string()),
            ("step", step),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .json::<PrometheusRangeData>()
        .await?;
    Ok(response)
}

pub async fn parse_prometheus_value(query: &str) -> f64 {
    match query_prometheus(query).await {
        Ok(data) => data
            .data
            .result
            .first()
            .and_then(|m| m.value.1.parse::<f64>().ok())
            .unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real Prometheus API response shape (from the live docs / actual
    /// query responses) — proves the parsing logic is correct without
    /// needing a reachable Prometheus (unavailable from a dev laptop; see
    /// module doc comment). Live verification happens in milestone 8.
    const SAMPLE_INSTANT_RESPONSE: &str = r#"{
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [
                { "metric": {"node": "pi5"}, "value": [1718000000, "42.5"] }
            ]
        }
    }"#;

    #[test]
    fn parses_real_prometheus_instant_response_shape() {
        let parsed: PrometheusData = serde_json::from_str(SAMPLE_INSTANT_RESPONSE).unwrap();
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.data.result.len(), 1);
        assert_eq!(parsed.data.result[0].metric.get("node").unwrap(), "pi5");
        assert_eq!(parsed.data.result[0].value.1.parse::<f64>().unwrap(), 42.5);
    }

    const SAMPLE_RANGE_RESPONSE: &str = r#"{
        "status": "success",
        "data": {
            "resultType": "matrix",
            "result": [
                { "metric": {}, "values": [[1718000000, "10.0"], [1718000600, "20.0"], [1718001200, "15.0"]] }
            ]
        }
    }"#;

    #[test]
    fn parses_real_prometheus_range_response_shape() {
        let parsed: PrometheusRangeData = serde_json::from_str(SAMPLE_RANGE_RESPONSE).unwrap();
        assert_eq!(parsed.data.result[0].values.len(), 3);
        let history: Vec<f64> = parsed.data.result[0]
            .values
            .iter()
            .map(|(_, v)| v.parse().unwrap())
            .collect();
        assert_eq!(history, vec![10.0, 20.0, 15.0]);
    }
}
