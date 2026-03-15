#[cfg(feature = "ssr")]
pub use inner::*;

#[cfg(feature = "ssr")]
mod inner {
    use std::collections::VecDeque;

    use serde::{Deserialize, Serialize};

    /// Rolling-window spike detector for cluster network tx.
    /// Window size and thresholds are tuned dynamically via Claude feedback.
    /// Cooldown / dedup is handled by the DB (spike_claims table), not in-memory,
    /// so multiple HA replicas don't each fire a separate explanation.
    pub struct NetworkSpikeDetector {
        tx_window: VecDeque<f64>,
        window_capacity: usize,
        min_samples: usize,
        pub multiplier: f64,
        pub floor_mbps: f64,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PodTraffic {
        pub namespace: String,
        pub pod: String,
        pub mbps: f64,
    }

    impl NetworkSpikeDetector {
        /// `window_capacity`: number of samples to keep (~3 min at your tick rate).
        /// `min_samples`: warmup samples before spike detection activates.
        pub fn new(multiplier: f64, floor_mbps: f64, window_capacity: usize, min_samples: usize) -> Self {
            Self {
                tx_window: VecDeque::with_capacity(window_capacity),
                window_capacity,
                min_samples,
                multiplier,
                floor_mbps,
            }
        }

        /// Feed the latest tx value. Returns `Some((spike, baseline))` when a
        /// spike is detected and the cooldown has elapsed.
        pub fn check(&mut self, tx_mbps: f64) -> Option<(f64, f64)> {
            if self.tx_window.len() >= self.window_capacity {
                self.tx_window.pop_front();
            }
            self.tx_window.push_back(tx_mbps);

            if self.tx_window.len() < self.min_samples {
                return None;
            }

            let n = self.tx_window.len();
            let baseline: f64 =
                self.tx_window.iter().take(n - 1).sum::<f64>() / (n - 1) as f64;

            if !(tx_mbps > baseline * self.multiplier && tx_mbps > self.floor_mbps) {
                return None;
            }

            Some((tx_mbps, baseline))
        }

        /// Apply significance feedback from Claude to nudge thresholds.
        ///
        /// - significance 1-3 (boring): raise floor 15% — we're too sensitive
        /// - significance 4-6: no change
        /// - significance 7-10 (real event): lower floor 10% — stay sensitive to similar events
        ///
        /// Clamps: floor 1.0–100.0 Mbps, multiplier 1.5–6.0x.
        pub fn apply_feedback(&mut self, significance: u8) {
            self.floor_mbps = match significance {
                1..=3 => (self.floor_mbps * 1.15).min(100.0),
                7..=10 => (self.floor_mbps * 0.90).max(1.0),
                _ => self.floor_mbps,
            };
        }
    }

    /// Query Prometheus for the top pods by tx at the moment of the spike,
    /// then call Claude to explain what happened.
    /// Returns `(top_pods, explanation, significance)` where significance is 1-10.
    pub async fn explain_spike(
        spike_tx_mbps: f64,
        baseline_tx_mbps: f64,
    ) -> Result<(Vec<PodTraffic>, String, u8), anyhow::Error> {
        use crate::prometheus_client::query_prometheus;

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        let empty_data = || crate::prometheus_client::PrometheusData {
            status: String::new(),
            data: crate::prometheus_client::PrometheusResult {
                result_type: String::new(),
                result: vec![],
            },
        };

        // Fan out all Prometheus queries concurrently.
        let (tx_data, rx_data, traefik_data, cilium_data) = tokio::join!(
            query_prometheus(
                "topk(5, sum by (namespace, pod) (rate(container_network_transmit_bytes_total[2m])))"
            ),
            query_prometheus(
                "topk(5, sum by (namespace, pod) (rate(container_network_receive_bytes_total[2m])))"
            ),
            query_prometheus(
                "topk(5, sum by (service) (rate(traefik_service_requests_total[2m])))"
            ),
            query_prometheus(
                "topk(10, sum by (reason, direction) (rate(cilium_drop_count_total[2m])))"
            ),
        );

        let tx_data = tx_data.unwrap_or_else(|_| empty_data());
        let rx_data = rx_data.unwrap_or_else(|_| empty_data());
        let traefik_data = traefik_data.unwrap_or_else(|_| empty_data());
        let cilium_data = cilium_data.unwrap_or_else(|_| empty_data());

        // Top TX pods (returned to caller for storage).
        let top_pods: Vec<PodTraffic> = tx_data
            .data
            .result
            .iter()
            .map(|m| PodTraffic {
                namespace: m.metric.get("namespace").cloned().unwrap_or_default(),
                pod: m.metric.get("pod").cloned().unwrap_or_default(),
                mbps: m.value.1.parse::<f64>().unwrap_or(0.0) * 8.0 / 1_000_000.0,
            })
            .filter(|p| !p.pod.is_empty())
            .collect();

        let fmt_pod_rows = |data: &crate::prometheus_client::PrometheusData| -> String {
            let rows: Vec<String> = data
                .data
                .result
                .iter()
                .map(|m| {
                    let ns = m.metric.get("namespace").cloned().unwrap_or_default();
                    let pod = m.metric.get("pod").cloned().unwrap_or_default();
                    let mbps = m.value.1.parse::<f64>().unwrap_or(0.0) * 8.0 / 1_000_000.0;
                    format!("  {ns}/{pod}: {mbps:.1} Mbps")
                })
                .filter(|s| !s.trim_start().starts_with('/'))
                .collect();
            if rows.is_empty() {
                "  (none)".to_string()
            } else {
                rows.join("\n")
            }
        };

        let traefik_section = {
            let rows: Vec<String> = traefik_data
                .data
                .result
                .iter()
                .map(|m| {
                    let svc = m.metric.get("service").cloned().unwrap_or_default();
                    let rps = m.value.1.parse::<f64>().unwrap_or(0.0);
                    format!("  {svc}: {rps:.1} req/s")
                })
                .filter(|s| !s.trim_start().starts_with('/'))
                .collect();
            if rows.is_empty() {
                "  (no Traefik data)".to_string()
            } else {
                rows.join("\n")
            }
        };

        let cilium_section = {
            let rows: Vec<String> = cilium_data
                .data
                .result
                .iter()
                .map(|m| {
                    let reason = m.metric.get("reason").cloned().unwrap_or_default();
                    let dir = m.metric.get("direction").cloned().unwrap_or_default();
                    let rate = m.value.1.parse::<f64>().unwrap_or(0.0);
                    format!("  {dir} {reason}: {rate:.1} drops/s")
                })
                .collect();
            if rows.is_empty() {
                "  (no Cilium drops)".to_string()
            } else {
                rows.join("\n")
            }
        };

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M %Z").to_string();

        let prompt = format!(
            "A network spike was detected in a homelab Kubernetes cluster \
             (self-hosted, rook-ceph storage, Traefik ingress, Cilium CNI, \
             mix of personal and infrastructure services).\n\n\
             Time: {now}\n\
             Baseline tx (3-min avg): {:.1} Mbps\n\
             Spike tx: {:.1} Mbps ({:.1}x increase)\n\n\
             Top pods by transmit:\n{}\n\n\
             Top pods by receive:\n{}\n\n\
             Traefik ingress request rate (top services):\n{traefik_section}\n\n\
             Cilium network drops:\n{cilium_section}\n\n\
             Respond with JSON only, no prose outside the JSON:\n\
             {{\"explanation\": \"2-3 sentence explanation of likely cause, whether concerning, and if action is needed\",\
             \"significance\": <integer 1-10 where 1=routine background noise, 10=critical/unexpected>}}",
            baseline_tx_mbps,
            spike_tx_mbps,
            spike_tx_mbps / baseline_tx_mbps.max(0.1),
            fmt_pod_rows(&tx_data),
            fmt_pod_rows(&rx_data),
        );

        let client = reqwest::Client::new();
        let res = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": "claude-haiku-4-5-20251001",
                "max_tokens": 256,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let raw = res["content"][0]["text"]
            .as_str()
            .unwrap_or("{}")
            .to_string();

        let parsed = serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_default();
        let explanation = parsed["explanation"]
            .as_str()
            .unwrap_or("Unable to generate explanation.")
            .to_string();
        let significance = parsed["significance"]
            .as_u64()
            .map(|v| v.clamp(1, 10) as u8)
            .unwrap_or(5);

        Ok((top_pods, explanation, significance))
    }
}
