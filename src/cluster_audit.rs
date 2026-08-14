#[cfg(feature = "ssr")]
pub use inner::*;

#[cfg(feature = "ssr")]
mod inner {
    /// Daily LLM pass over cluster telemetry — a second opinion layered on top of
    /// the Prometheus threshold alerts (CPU-anomaly, mining-port-egress). Those
    /// catch what we thought to define; this is meant to catch what we didn't:
    /// odd combinations, drift, things that don't trip a fixed threshold but
    /// look wrong together. Non-deterministic by nature, so treat findings as
    /// leads to check, not verdicts.
    use crate::prometheus_client::query_prometheus;

    /// Run one audit pass: gather a Prometheus digest, ask Claude to review it,
    /// store the result. Returns `(summary, significance)` on success.
    pub async fn run_audit(
        pool: Option<&sqlx::PgPool>,
    ) -> Result<(String, u8), anyhow::Error> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        let empty_data = || crate::prometheus_client::PrometheusData {
            status: String::new(),
            data: crate::prometheus_client::PrometheusResult {
                result_type: String::new(),
                result: vec![],
            },
        };

        let (firing_alerts, cpu_top, net_tx_top, cilium_drops, mining_ports) = tokio::join!(
            query_prometheus(r#"ALERTS{alertstate="firing"}"#),
            query_prometheus(
                "topk(8, sum by (namespace, pod) (rate(container_cpu_usage_seconds_total{container!=\"\"}[1h])))"
            ),
            query_prometheus(
                "topk(8, sum by (namespace, pod) (rate(container_network_transmit_bytes_total[1h])))"
            ),
            query_prometheus(
                "topk(10, sum by (reason, direction) (rate(cilium_drop_count_total[24h])))"
            ),
            query_prometheus(
                r#"hubble_port_distribution_total{port=~"3333|4444|5555|7777|9999|14444|14433"}"#
            ),
        );

        let firing_alerts = firing_alerts.unwrap_or_else(|_| empty_data());
        let cpu_top = cpu_top.unwrap_or_else(|_| empty_data());
        let net_tx_top = net_tx_top.unwrap_or_else(|_| empty_data());
        let cilium_drops = cilium_drops.unwrap_or_else(|_| empty_data());
        let mining_ports = mining_ports.unwrap_or_else(|_| empty_data());

        let fmt_alerts = || -> String {
            let rows: Vec<String> = firing_alerts
                .data
                .result
                .iter()
                .map(|m| {
                    let name = m.metric.get("alertname").cloned().unwrap_or_default();
                    let ns = m.metric.get("namespace").cloned().unwrap_or_default();
                    let summary = m.metric.get("summary").cloned().unwrap_or_default();
                    format!("  {name} [{ns}]: {summary}")
                })
                .collect();
            if rows.is_empty() { "  (none firing)".to_string() } else { rows.join("\n") }
        };

        let fmt_pod_rows = |data: &crate::prometheus_client::PrometheusData, unit: &str, scale: f64| -> String {
            let rows: Vec<String> = data
                .data
                .result
                .iter()
                .map(|m| {
                    let ns = m.metric.get("namespace").cloned().unwrap_or_default();
                    let pod = m.metric.get("pod").cloned().unwrap_or_default();
                    let v = m.value.1.parse::<f64>().unwrap_or(0.0) * scale;
                    format!("  {ns}/{pod}: {v:.2} {unit}")
                })
                .filter(|s| !s.trim_start().starts_with('/'))
                .collect();
            if rows.is_empty() { "  (none)".to_string() } else { rows.join("\n") }
        };

        let cilium_section = {
            let rows: Vec<String> = cilium_drops
                .data
                .result
                .iter()
                .map(|m| {
                    let reason = m.metric.get("reason").cloned().unwrap_or_default();
                    let dir = m.metric.get("direction").cloned().unwrap_or_default();
                    let rate = m.value.1.parse::<f64>().unwrap_or(0.0);
                    format!("  {dir} {reason}: {rate:.2} drops/s (24h avg)")
                })
                .collect();
            if rows.is_empty() { "  (none)".to_string() } else { rows.join("\n") }
        };

        let mining_section = {
            let rows: Vec<String> = mining_ports
                .data
                .result
                .iter()
                .map(|m| {
                    let node = m.metric.get("node").cloned().unwrap_or_default();
                    let port = m.metric.get("port").cloned().unwrap_or_default();
                    let proto = m.metric.get("protocol").cloned().unwrap_or_default();
                    let count = m.value.1.clone();
                    format!("  node={node} port={port}/{proto} cumulative_count={count}")
                })
                .collect();
            if rows.is_empty() { "  (no traffic seen on known Stratum/mining-pool ports)".to_string() } else { rows.join("\n") }
        };

        let recent_insights = match pool {
            Some(p) => crate::db::get_recent_network_insights(p, 5).await.unwrap_or_default(),
            None => vec![],
        };
        let insights_section = if recent_insights.is_empty() {
            "  (none in the last few days)".to_string()
        } else {
            recent_insights
                .iter()
                .map(|i| format!("  [{}] {:.0}x baseline: {}", i.occurred_at.format("%Y-%m-%d %H:%M"), i.spike_tx_mbps / i.baseline_tx_mbps.max(0.1), i.explanation))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M %Z").to_string();

        let prompt = format!(
            "Daily security review of a homelab Kubernetes cluster (self-hosted, \
             rook-ceph storage, Traefik ingress, Cilium CNI, mix of personal apps \
             and infra services). This is a second-opinion pass — fixed threshold \
             alerts already watch for known bad patterns (CPU spikes vs baseline, \
             traffic to known mining-pool ports). You're looking for anything odd \
             that a fixed threshold wouldn't catch: unusual combinations, drift, \
             a pod that doesn't belong, resource use that doesn't match its job.\n\n\
             Time: {now}\n\n\
             Currently firing Prometheus alerts:\n{}\n\n\
             Top CPU consumers (1h rate, cores):\n{}\n\n\
             Top network transmitters (1h rate):\n{}\n\n\
             Cilium drop reasons (24h):\n{cilium_section}\n\n\
             Traffic on known Stratum/mining-pool ports (cumulative counters, any \
             nonzero value here is worth flagging):\n{mining_section}\n\n\
             Recent auto-explained network spikes (last few days):\n{insights_section}\n\n\
             Respond with JSON only, no prose outside the JSON:\n\
             {{\"summary\": \"1-2 sentence overall assessment\", \
             \"significance\": <integer 1-10 where 1=nothing notable, 10=likely compromise>, \
             \"findings\": [{{\"title\": \"short title\", \"detail\": \"1 sentence\"}}]}}\n\
             Empty findings array is fine and expected most days.",
            fmt_alerts(),
            fmt_pod_rows(&cpu_top, "cores", 1.0),
            fmt_pod_rows(&net_tx_top, "Mbps", 8.0 / 1_000_000.0),
        );

        const MODEL: &str = "claude-haiku-4-5-20251001";

        let client = reqwest::Client::new();
        let api_result = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": MODEL,
                "max_tokens": 768,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await;

        match &api_result {
            Ok(res) => {
                let raw_text = res["content"][0]["text"].as_str().unwrap_or("").to_string();
                let input_tokens = res["usage"]["input_tokens"].as_i64().map(|v| v as i32);
                let output_tokens = res["usage"]["output_tokens"].as_i64().map(|v| v as i32);
                if let Some(p) = pool {
                    let _ = crate::db::insert_claude_audit(
                        p,
                        "cluster_audit",
                        MODEL,
                        &prompt,
                        Some(&raw_text),
                        input_tokens,
                        output_tokens,
                        None,
                    )
                    .await;
                }
            }
            Err(e) => {
                if let Some(p) = pool {
                    let _ = crate::db::insert_claude_audit(
                        p,
                        "cluster_audit",
                        MODEL,
                        &prompt,
                        None,
                        None,
                        None,
                        Some(&e.to_string()),
                    )
                    .await;
                }
            }
        }

        let res = api_result?;
        let raw = res["content"][0]["text"].as_str().unwrap_or("{}").to_string();
        let json_str = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let parsed = serde_json::from_str::<serde_json::Value>(json_str).unwrap_or_default();
        let summary = parsed["summary"]
            .as_str()
            .unwrap_or("Unable to generate summary.")
            .to_string();
        let significance = parsed["significance"]
            .as_u64()
            .map(|v| v.clamp(1, 10) as u8)
            .unwrap_or(5);
        let findings = parsed
            .get("findings")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(vec![]));

        if let Some(p) = pool {
            let _ = crate::db::insert_cluster_audit(p, &summary, significance as i16, &findings).await;
        }

        Ok((summary, significance))
    }
}
