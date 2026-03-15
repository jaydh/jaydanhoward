#[cfg(feature = "ssr")]
pub use inner::*;

#[cfg(feature = "ssr")]
mod inner {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    use serde::{Deserialize, Serialize};

    /// Rolling-window spike detector for cluster network tx.
    /// Holds 36 samples (3 minutes at 5s SSE interval).
    pub struct NetworkSpikeDetector {
        tx_window: VecDeque<f64>,
        last_spike_at: Option<Instant>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PodTraffic {
        pub namespace: String,
        pub pod: String,
        pub mbps: f64,
    }

    impl NetworkSpikeDetector {
        pub fn new() -> Self {
            Self {
                tx_window: VecDeque::with_capacity(36),
                last_spike_at: None,
            }
        }

        /// Feed the latest tx value. Returns `Some((spike, baseline))` when a
        /// spike is detected and the cooldown has elapsed.
        pub fn check(&mut self, tx_mbps: f64) -> Option<(f64, f64)> {
            if self.tx_window.len() >= 36 {
                self.tx_window.pop_front();
            }
            self.tx_window.push_back(tx_mbps);

            // Need at least 6 samples (~30s) before declaring a spike.
            if self.tx_window.len() < 6 {
                return None;
            }

            let n = self.tx_window.len();
            let baseline: f64 =
                self.tx_window.iter().take(n - 1).sum::<f64>() / (n - 1) as f64;

            let is_spike = tx_mbps > baseline * 2.5 && tx_mbps > 50.0;
            if !is_spike {
                return None;
            }

            // 5-minute cooldown so one sustained event doesn't spam.
            if let Some(last) = self.last_spike_at {
                if last.elapsed() < Duration::from_secs(300) {
                    return None;
                }
            }

            self.last_spike_at = Some(Instant::now());
            Some((tx_mbps, baseline))
        }
    }

    /// Query Prometheus for the top pods by tx at the moment of the spike,
    /// then call Claude to explain what happened.
    pub async fn explain_spike(
        spike_tx_mbps: f64,
        baseline_tx_mbps: f64,
    ) -> Result<(Vec<PodTraffic>, String), anyhow::Error> {
        use crate::prometheus_client::query_prometheus;

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        // Enrich: top pods by tx over the last 2 minutes.
        let pod_data = query_prometheus(
            "topk(5, sum by (namespace, pod) (rate(container_network_transmit_bytes_total[2m])))",
        )
        .await
        .unwrap_or_else(|_| crate::prometheus_client::PrometheusData {
            status: String::new(),
            data: crate::prometheus_client::PrometheusResult {
                result_type: String::new(),
                result: vec![],
            },
        });

        let top_pods: Vec<PodTraffic> = pod_data
            .data
            .result
            .iter()
            .map(|m| PodTraffic {
                namespace: m.metric.get("namespace").cloned().unwrap_or_default(),
                pod: m.metric.get("pod").cloned().unwrap_or_default(),
                // Prometheus gives bytes/s — convert to Mbps.
                mbps: m.value.1.parse::<f64>().unwrap_or(0.0) * 8.0 / 1_000_000.0,
            })
            .filter(|p| !p.pod.is_empty())
            .collect();

        let pod_lines: Vec<String> = top_pods
            .iter()
            .map(|p| format!("  {}/{}: {:.1} Mbps", p.namespace, p.pod, p.mbps))
            .collect();

        let prompt = format!(
            "A network spike was detected in a homelab Kubernetes cluster \
             (self-hosted, rook-ceph storage, mix of personal and infrastructure services).\n\n\
             Baseline tx (3-min avg): {:.1} Mbps\n\
             Spike tx: {:.1} Mbps ({:.1}x increase)\n\n\
             Top pods by transmit at time of spike:\n\
             {}\n\n\
             In 2-3 sentences: what likely caused this spike, is it concerning, \
             and is any action needed?",
            baseline_tx_mbps,
            spike_tx_mbps,
            spike_tx_mbps / baseline_tx_mbps.max(0.1),
            if pod_lines.is_empty() {
                "  (no per-pod data available)".to_string()
            } else {
                pod_lines.join("\n")
            },
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

        let explanation = res["content"][0]["text"]
            .as_str()
            .unwrap_or("Unable to generate explanation.")
            .to_string();

        Ok((top_pods, explanation))
    }
}
