//! Daily LLM pass over cluster telemetry — a second opinion layered on top
//! of the Prometheus threshold alerts (CPU-anomaly, mining-port-egress; see
//! homelab repo's monitoring/prometheus.yaml). Those catch what was
//! specifically defined; this is meant to catch what wasn't: odd
//! combinations, drift, things that don't trip a fixed threshold but look
//! wrong together. Non-deterministic by nature — findings are leads to
//! check, not verdicts. Ported from `src/cluster_audit.rs`, adapted to this
//! crate's `prometheus_client` and writing directly to Postgres instead of
//! going through Leptos server functions.

use crate::prometheus_client::{empty_data, query_prometheus, PrometheusData};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};

/// Try to atomically claim the right to run today's cluster audit. Returns
/// true if this replica won the race, false if another replica already
/// claimed today's bucket (or on DB error).
pub async fn try_claim_cluster_audit(pool: &PgPool) -> bool {
    let result = sqlx::query(
        "INSERT INTO cluster_audit_claims (bucket) \
         VALUES (date_trunc('day', NOW())) \
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await;

    matches!(result, Ok(r) if r.rows_affected() == 1)
}

async fn insert_cluster_audit(pool: &PgPool, summary: &str, significance: i16, findings: &Value) {
    let _ = sqlx::query(
        "INSERT INTO cluster_audits (summary, significance, findings) VALUES ($1, $2, $3)",
    )
    .bind(summary)
    .bind(significance)
    .bind(findings)
    .execute(pool)
    .await;
}

/// Recent audit runs, newest first — feeds the "Daily Audit" panel in the
/// live Cluster snapshot (see cluster.rs::fetch_cluster_snapshot).
pub async fn fetch_cluster_audits(pool: &PgPool) -> Vec<Value> {
    let rows = sqlx::query(
        "SELECT occurred_at, summary, significance, findings \
         FROM cluster_audits ORDER BY occurred_at DESC LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            let occurred_at: chrono::DateTime<chrono::Utc> = r.try_get("occurred_at").unwrap_or_default();
            json!({
                "occurred_at": occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                "summary": r.try_get::<String, _>("summary").unwrap_or_default(),
                "significance": r.try_get::<i16, _>("significance").unwrap_or(0),
                "findings": r.try_get::<Value, _>("findings").unwrap_or(Value::Array(vec![])),
            })
        })
        .collect()
}

fn insert_claude_audit_query<'a>(
    context: &'a str,
    model: &'a str,
    prompt: &'a str,
    response: Option<&'a str>,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    error: Option<&'a str>,
) -> sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments> {
    sqlx::query(
        "INSERT INTO claude_audit_log \
         (context, model, prompt, response, input_tokens, output_tokens, error) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(context)
    .bind(model)
    .bind(prompt)
    .bind(response)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(error)
}

/// Run one audit pass: gather a Prometheus digest, ask Claude to review it,
/// store the result. Returns `(summary, significance)` on success.
pub async fn run_audit(pool: &PgPool) -> Result<(String, u8), anyhow::Error> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

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

    let fmt_pod_rows = |data: &PrometheusData, unit: &str, scale: f64| -> String {
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
        .json(&json!({
            "model": MODEL,
            "max_tokens": 768,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await?
        .json::<Value>()
        .await;

    match &api_result {
        Ok(res) => {
            let raw_text = res["content"][0]["text"].as_str().unwrap_or("").to_string();
            let input_tokens = res["usage"]["input_tokens"].as_i64().map(|v| v as i32);
            let output_tokens = res["usage"]["output_tokens"].as_i64().map(|v| v as i32);
            let _ = insert_claude_audit_query(
                "cluster_audit",
                MODEL,
                &prompt,
                Some(&raw_text),
                input_tokens,
                output_tokens,
                None,
            )
            .execute(pool)
            .await;
        }
        Err(e) => {
            let _ = insert_claude_audit_query(
                "cluster_audit",
                MODEL,
                &prompt,
                None,
                None,
                None,
                Some(&e.to_string()),
            )
            .execute(pool)
            .await;
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

    let parsed = serde_json::from_str::<Value>(json_str).unwrap_or_default();
    let summary = parsed["summary"].as_str().unwrap_or("Unable to generate summary.").to_string();
    let significance = parsed["significance"].as_u64().map(|v| v.clamp(1, 10) as u8).unwrap_or(5);
    let findings = parsed.get("findings").cloned().unwrap_or_else(|| Value::Array(vec![]));

    insert_cluster_audit(pool, &summary, significance as i16, &findings).await;

    Ok((summary, significance))
}
