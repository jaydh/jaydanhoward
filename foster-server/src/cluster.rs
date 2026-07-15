//! Full Prometheus-backed cluster panel — ported from the real
//! `src/components/cluster_stats.rs` (2375 lines covering 10+ panels).
//! Query strings and parsing logic below are copied verbatim from there;
//! this module just re-shapes the results into one JSON context object for
//! a single "cluster" Foster machine instead of a dozen separate Leptos
//! server functions.
//!
//! PROMETHEUS_URL is unreachable from this dev laptop (confirmed: its
//! NodePort is bound to the home LAN interface, this machine connects over
//! Tailscale) but works fine from the production pod itself, which gets
//! normal in-cluster network access — every query below degrades to zeros
//! gracefully when the env var is unset or the request fails, and
//! `prometheus_client`'s unit tests prove the parsing logic is correct
//! against real Prometheus response shapes. Live verification against a
//! real Prometheus happens in milestone 8's in-cluster staging step.

use crate::prometheus_client::{empty_data, parse_prometheus_value, query_prometheus, query_prometheus_range};
use axum::extract::State;
use axum::http::StatusCode;
use k8s_openapi::api::batch::v1::Job;
use kube::api::{Api, ListParams};
use kube::core::DynamicObject;
use kube::discovery::ApiResource;
use kube::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

/// Real ingest endpoint for external Claude usage tracking, ported
/// verbatim from `src/routes/audit.rs` — no auth (matches production; this
/// is a same-network/internal reporting channel, not user-facing).
#[derive(Deserialize)]
pub struct ClaudeAuditPayload {
    pub context: String,
    pub model: String,
    pub prompt: String,
    pub response: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub error: Option<String>,
}

pub async fn ingest_claude_audit(
    State(pool): State<PgPool>,
    axum::Json(payload): axum::Json<ClaudeAuditPayload>,
) -> StatusCode {
    let result = sqlx::query(
        "INSERT INTO claude_audit_log \
         (context, model, prompt, response, input_tokens, output_tokens, error) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&payload.context)
    .bind(&payload.model)
    .bind(&payload.prompt)
    .bind(&payload.response)
    .bind(payload.input_tokens)
    .bind(payload.output_tokens)
    .bind(&payload.error)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// ── Cluster metrics (CPU/mem/disk/pod/node + storage) ──────────────────────

async fn fetch_cluster_metrics() -> Value {
    let cpu_used = parse_prometheus_value("sum(rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))").await;
    let cpu_total = parse_prometheus_value("sum(machine_cpu_cores)").await;
    let memory_used = parse_prometheus_value("sum(container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024").await;
    let memory_total = parse_prometheus_value("sum(machine_memory_bytes) / 1024 / 1024 / 1024").await;
    let disk_used = parse_prometheus_value("ceph_cluster_total_used_bytes / 1024 / 1024 / 1024").await;
    let disk_total = parse_prometheus_value("ceph_cluster_total_bytes / 1024 / 1024 / 1024").await;
    let pod_count = parse_prometheus_value("count(kube_pod_info{pod!=\"\"})").await as i64;
    let node_count = parse_prometheus_value("count(kube_node_info)").await as i64;
    let healthy_node_count = parse_prometheus_value("sum(kube_node_status_condition{condition=\"Ready\",status=\"true\"})").await as i64;
    let network_rx = parse_prometheus_value(
        "sum(rate(node_network_receive_bytes_total{device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"}[5m])) * 8 / 1000000",
    )
    .await;
    let network_tx = parse_prometheus_value(
        "sum(rate(node_network_transmit_bytes_total{device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"}[5m])) * 8 / 1000000",
    )
    .await;

    let (cap_data, used_data) = tokio::join!(
        query_prometheus("max by (namespace, persistentvolumeclaim) (kubelet_volume_stats_capacity_bytes)"),
        query_prometheus("max by (namespace, persistentvolumeclaim) (kubelet_volume_stats_used_bytes)"),
    );
    let cap_map: HashMap<(String, String), i64> = cap_data
        .unwrap_or_else(|_| empty_data())
        .data
        .result
        .into_iter()
        .filter_map(|m| {
            let ns = m.metric.get("namespace")?.clone();
            let pvc = m.metric.get("persistentvolumeclaim")?.clone();
            let bytes = m.value.1.parse::<f64>().ok()? as i64;
            Some(((ns, pvc), bytes))
        })
        .collect();
    let mut pvcs: Vec<Value> = used_data
        .unwrap_or_else(|_| empty_data())
        .data
        .result
        .into_iter()
        .filter_map(|m| {
            let ns = m.metric.get("namespace")?.clone();
            let pvc = m.metric.get("persistentvolumeclaim")?.clone();
            let used = m.value.1.parse::<f64>().ok()? as i64;
            let capacity = *cap_map.get(&(ns.clone(), pvc.clone())).unwrap_or(&0);
            Some(json!({ "namespace": ns, "name": pvc, "used_bytes": used, "capacity_bytes": capacity }))
        })
        .collect();
    pvcs.sort_by(|a, b| b["used_bytes"].as_i64().cmp(&a["used_bytes"].as_i64()));

    json!({
        "cpu_usage_percent": (cpu_used / cpu_total.max(0.001) * 100.0).min(100.0),
        "cpu_total_cores": cpu_total,
        "memory_usage_gb": memory_used,
        "memory_total_gb": memory_total,
        "disk_usage_gb": disk_used,
        "disk_total_gb": disk_total,
        "pod_count": pod_count,
        "node_count": node_count,
        "healthy_node_count": healthy_node_count,
        "network_rx_mbps": network_rx,
        "network_tx_mbps": network_tx,
        "pvcs": pvcs,
    })
}

// ── Per-node metrics ────────────────────────────────────────────────────────

async fn fetch_node_metrics() -> Vec<Value> {
    let cpu_data = query_prometheus("sum by (node) (rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))").await;
    let memory_data = query_prometheus("sum by (node) (container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024").await;
    let memory_total_data = query_prometheus("sum by (node) (machine_memory_bytes) / 1024 / 1024 / 1024").await;
    let cpu_capacity_data = query_prometheus("sum by (node) (machine_cpu_cores)").await;

    let mut nodes: HashMap<String, (f64, f64, f64)> = HashMap::new(); // cpu_used, mem_used, mem_total

    if let Ok(d) = &cpu_data {
        for m in &d.data.result {
            if let Some(node) = m.metric.get("node") {
                nodes.entry(node.clone()).or_insert((0.0, 0.0, 0.0)).0 = m.value.1.parse().unwrap_or(0.0);
            }
        }
    }
    if let Ok(d) = &memory_data {
        for m in &d.data.result {
            if let Some(node) = m.metric.get("node") {
                if let Some(entry) = nodes.get_mut(node) {
                    entry.1 = m.value.1.parse().unwrap_or(0.0);
                }
            }
        }
    }
    if let Ok(d) = &memory_total_data {
        for m in &d.data.result {
            if let Some(node) = m.metric.get("node") {
                if let Some(entry) = nodes.get_mut(node) {
                    entry.2 = m.value.1.parse().unwrap_or(0.0);
                }
            }
        }
    }

    let mut cpu_capacity: HashMap<String, f64> = HashMap::new();
    if let Ok(d) = &cpu_capacity_data {
        for m in &d.data.result {
            if let Some(node) = m.metric.get("node") {
                cpu_capacity.insert(node.clone(), m.value.1.parse().unwrap_or(1.0));
            }
        }
    }

    nodes
        .into_iter()
        .map(|(name, (cpu_used, mem_used, mem_total))| {
            let cpu_total = *cpu_capacity.get(&name).unwrap_or(&1.0);
            json!({
                "name": name,
                "cpu_usage_percent": (cpu_used / cpu_total.max(0.001) * 100.0).min(100.0),
                "memory_usage_gb": mem_used,
                "memory_total_gb": mem_total,
            })
        })
        .collect()
}

// ── Historical (24h, 10m step) ──────────────────────────────────────────────

/// Returns (full series for the sparkline widget, min/avg/max/latest
/// summary) — the full series is nested under a top-level "series" key read
/// by a small canvas widget via the same data-fx-item JS pattern
/// satellites.js/photography.js already use for structured (non-scalar)
/// list data; the summary scalars are flattened to the context root so
/// fx-text can bind them directly (Foster's fx-* attribute lookups are a
/// single top-level ctx[key], not a dotted path).
async fn fetch_historical_metrics() -> (Value, Value) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let start = now - 24 * 3600;
    let step = "10m";

    let extract = |r: Result<crate::prometheus_client::PrometheusRangeData, anyhow::Error>| -> Vec<f64> {
        r.ok()
            .and_then(|d| d.data.result.into_iter().next())
            .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
            .unwrap_or_default()
    };

    let (cpu, mem, disk, rx, tx) = tokio::join!(
        query_prometheus_range("sum(rate(container_cpu_usage_seconds_total{container!=\"\"}[5m])) / sum(machine_cpu_cores) * 100", start, now, step),
        query_prometheus_range("sum(container_memory_working_set_bytes{container!=\"\"}) / sum(machine_memory_bytes) * 100", start, now, step),
        query_prometheus_range("ceph_cluster_total_used_bytes / ceph_cluster_total_bytes * 100", start, now, step),
        query_prometheus_range("sum(rate(node_network_receive_bytes_total{device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"}[5m])) * 8 / 1000000", start, now, step),
        query_prometheus_range("sum(rate(node_network_transmit_bytes_total{device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"}[5m])) * 8 / 1000000", start, now, step),
    );

    let cpu_history = extract(cpu);
    let memory_history = extract(mem);
    let disk_history = extract(disk);
    let network_rx_history = extract(rx);
    let network_tx_history = extract(tx);

    fn summarize(series: &[f64]) -> (f64, f64) {
        let latest = series.last().copied().unwrap_or(0.0);
        let max = series.iter().cloned().fold(0.0_f64, f64::max);
        (latest, max)
    }
    let (cpu_latest, cpu_max) = summarize(&cpu_history);
    let (mem_latest, mem_max) = summarize(&memory_history);
    let (disk_latest, disk_max) = summarize(&disk_history);

    let series = json!({
        "cpu": cpu_history, "memory": memory_history, "disk": disk_history,
        "network_rx": network_rx_history, "network_tx": network_tx_history,
    });
    let summary = json!({
        "cpu_history_latest": cpu_latest, "cpu_history_max": cpu_max,
        "memory_history_latest": mem_latest, "memory_history_max": mem_max,
        "disk_history_latest": disk_latest, "disk_history_max": disk_max,
    });
    (series, summary)
}

// ── Ceph status ──────────────────────────────────────────────────────────────

async fn fetch_ceph_status() -> Value {
    let health = parse_prometheus_value("ceph_health_status").await as i64;
    let mon_quorum = parse_prometheus_value("sum(ceph_mon_quorum_status == 1)").await as i64;
    let mon_total = parse_prometheus_value("count(ceph_mon_quorum_status)").await as i64;
    let mgr_active = parse_prometheus_value("sum(ceph_mgr_status == 1)").await as i64;
    let mgr_standby = parse_prometheus_value("sum(ceph_mgr_status == 0)").await as i64;
    let mds_up = parse_prometheus_value("count(ceph_mds_metadata{fs_state=\"up:active\"})").await as i64;
    let mds_standby = parse_prometheus_value("count(ceph_mds_metadata{fs_state=~\"up:standby.*\"})").await as i64;
    let osd_up = parse_prometheus_value("count(ceph_osd_up == 1)").await as i64;
    let osd_in = parse_prometheus_value("count(ceph_osd_in == 1)").await as i64;
    let osd_total = parse_prometheus_value("count(ceph_osd_up)").await as i64;
    let rgw_count = parse_prometheus_value("count(ceph_rgw_metadata)").await as i64;
    let volumes_total = parse_prometheus_value("count(ceph_fs_metadata)").await as i64;
    let pool_count = parse_prometheus_value("ceph_osdmap_num_pools").await as i64;
    let pg_total = parse_prometheus_value("ceph_osdmap_num_pg").await as i64;
    let pg_clean = parse_prometheus_value("ceph_pg_state{state=\"active+clean\"}").await as i64;
    let pg_degraded = parse_prometheus_value("sum(ceph_pg_state{state=~\".*degraded.*\"})").await as i64;
    let pg_recovering = parse_prometheus_value("sum(ceph_pg_state{state=~\".*recovering.*\"})").await as i64;
    let pg_remapped = parse_prometheus_value("sum(ceph_pg_state{state=~\".*remapped.*\"})").await as i64;
    let pg_scrubbing = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing(?!\\+deep).*\"})").await as i64;
    let pg_deep_scrub = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing\\+deep.*\"})").await as i64;
    let objects_count = parse_prometheus_value("sum(ceph_pool_objects_total)").await;
    let data_used_bytes = parse_prometheus_value("ceph_cluster_total_used_bytes").await;
    let data_total_bytes = parse_prometheus_value("ceph_cluster_total_bytes").await;
    let read_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_r_out_bytes[5m]))").await;
    let write_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_w_in_bytes[5m]))").await;
    let read_iops = parse_prometheus_value("sum(irate(ceph_osd_op_r[5m]))").await;
    let write_iops = parse_prometheus_value("sum(irate(ceph_osd_op_w[5m]))").await;

    json!({
        "health": health, "mon_quorum": mon_quorum, "mon_total": mon_total,
        "mgr_active": mgr_active, "mgr_standby": mgr_standby,
        "mds_up": mds_up, "mds_standby": mds_standby,
        "osd_up": osd_up, "osd_in": osd_in, "osd_total": osd_total, "rgw_count": rgw_count,
        "volumes_healthy": volumes_total, "volumes_total": volumes_total,
        "pool_count": pool_count, "pg_total": pg_total, "pg_clean": pg_clean,
        "pg_degraded": pg_degraded, "pg_recovering": pg_recovering, "pg_remapped": pg_remapped,
        "pg_scrubbing": pg_scrubbing, "pg_deep_scrub": pg_deep_scrub,
        "objects_count": objects_count,
        "data_used_bytes": data_used_bytes, "data_total_bytes": data_total_bytes,
        "data_avail_bytes": (data_total_bytes - data_used_bytes).max(0.0),
        "read_bytes_per_sec": read_bytes_per_sec, "write_bytes_per_sec": write_bytes_per_sec,
        "read_iops": read_iops, "write_iops": write_iops,
    })
}

// ── Top network pods + external breakdown + cloudflared ────────────────────

async fn fetch_top_network_pods() -> Vec<Value> {
    let (tx_data, rx_data) = tokio::join!(
        query_prometheus("topk(10, sum by (namespace, pod) (rate(container_network_transmit_bytes_total{pod!=\"\",namespace!~\"kube-system|monitoring|ingress\"}[5m]))) * 8 / 1000000"),
        query_prometheus("topk(10, sum by (namespace, pod) (rate(container_network_receive_bytes_total{pod!=\"\",namespace!~\"kube-system|monitoring|ingress\"}[5m]))) * 8 / 1000000"),
    );
    let mut map: HashMap<(String, String), (f64, f64)> = HashMap::new();
    for m in tx_data.unwrap_or_else(|_| empty_data()).data.result {
        let ns = m.metric.get("namespace").cloned().unwrap_or_default();
        let pod = m.metric.get("pod").cloned().unwrap_or_default();
        if pod.is_empty() { continue; }
        map.entry((ns, pod)).or_insert((0.0, 0.0)).0 = m.value.1.parse().unwrap_or(0.0);
    }
    for m in rx_data.unwrap_or_else(|_| empty_data()).data.result {
        let ns = m.metric.get("namespace").cloned().unwrap_or_default();
        let pod = m.metric.get("pod").cloned().unwrap_or_default();
        if pod.is_empty() { continue; }
        map.entry((ns, pod)).or_insert((0.0, 0.0)).1 = m.value.1.parse().unwrap_or(0.0);
    }
    let mut pods: Vec<Value> = map
        .into_iter()
        .map(|((namespace, pod), (tx_mbps, rx_mbps))| json!({ "namespace": namespace, "pod": pod, "tx_mbps": tx_mbps, "rx_mbps": rx_mbps }))
        .collect();
    pods.sort_by(|a, b| {
        let sa = a["tx_mbps"].as_f64().unwrap_or(0.0) + a["rx_mbps"].as_f64().unwrap_or(0.0);
        let sb = b["tx_mbps"].as_f64().unwrap_or(0.0) + b["rx_mbps"].as_f64().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    pods.truncate(10);
    pods
}

async fn fetch_cloudflared_status() -> Value {
    let (ha, total_reqs, code_reqs, errors) = tokio::join!(
        query_prometheus("sum(cloudflared_tunnel_ha_connections)"),
        query_prometheus("sum(rate(cloudflared_tunnel_total_requests[5m]))"),
        query_prometheus("sum by (status_code) (rate(cloudflared_tunnel_response_by_code[5m]))"),
        query_prometheus("sum(rate(cloudflared_tunnel_request_errors[5m]))"),
    );
    let scalar = |d: Result<crate::prometheus_client::PrometheusData, anyhow::Error>| -> f64 {
        d.unwrap_or_else(|_| empty_data()).data.result.first().and_then(|m| m.value.1.parse::<f64>().ok()).unwrap_or(0.0)
    };
    let mut by_status: Vec<Value> = code_reqs
        .unwrap_or_else(|_| empty_data())
        .data
        .result
        .into_iter()
        .filter_map(|m| {
            let status_code = m.metric.get("status_code")?.clone();
            let rps = m.value.1.parse::<f64>().ok()?;
            Some(json!({ "status_code": status_code, "req_per_sec": rps }))
        })
        .collect();
    by_status.sort_by(|a, b| b["req_per_sec"].as_f64().partial_cmp(&a["req_per_sec"].as_f64()).unwrap());

    json!({
        "ha_connections": scalar(ha) as i64,
        "total_req_per_sec": scalar(total_reqs),
        "error_rate": scalar(errors),
        "by_status": by_status,
    })
}

// ── GitOps (kube) + backup Jobs (kube) — same as the earlier PoC ───────────

const FLUX_TYPES: &[(&str, &str, &str, &str)] = &[
    ("Kustomization", "kustomize.toolkit.fluxcd.io", "kustomizations", "v1"),
    ("HelmRelease", "helm.toolkit.fluxcd.io", "helmreleases", "v2"),
    ("GitRepository", "source.toolkit.fluxcd.io", "gitrepositories", "v1"),
    ("HelmRepository", "source.toolkit.fluxcd.io", "helmrepositories", "v1"),
    ("HelmChart", "source.toolkit.fluxcd.io", "helmcharts", "v1"),
];

const BACKUP_JOBS: &[(&str, &str)] = &[
    ("backup-immich-photos", "immich photos"),
    ("backup-media", "media"),
    ("backup-backup-vol", "backup vol"),
];

async fn fetch_gitops_status(client: &Client) -> Vec<Value> {
    let mut resources = Vec::new();
    for &(kind, group, plural, version) in FLUX_TYPES {
        let ar = ApiResource {
            group: group.to_string(),
            version: version.to_string(),
            api_version: format!("{group}/{version}"),
            kind: kind.to_string(),
            plural: plural.to_string(),
        };
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
        let list = match api.list(&ListParams::default()).await {
            Ok(l) => l,
            Err(_) => continue,
        };
        for obj in list.items {
            let name = obj.metadata.name.unwrap_or_default();
            let namespace = obj.metadata.namespace.unwrap_or_default();
            let ready = obj.data["status"]["conditions"]
                .as_array()
                .and_then(|c| c.iter().find(|c| c["type"].as_str() == Some("Ready")))
                .and_then(|c| c["status"].as_str())
                .map(|s| s == "True")
                .unwrap_or(false);
            resources.push(json!({ "kind": kind, "namespace": namespace, "name": name, "ready_icon": if ready { "\u{2713}" } else { "\u{2717}" } }));
        }
    }
    resources.sort_by(|a, b| {
        a["ready_icon"].as_str().cmp(&b["ready_icon"].as_str())
            .then(a["kind"].as_str().cmp(&b["kind"].as_str()))
            .then(a["name"].as_str().cmp(&b["name"].as_str()))
    });
    resources
}

async fn fetch_backup_status(client: &Client) -> Vec<Value> {
    let jobs: Api<Job> = Api::namespaced(client.clone(), "media");
    let list = match jobs.list(&ListParams::default()).await {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    BACKUP_JOBS
        .iter()
        .map(|&(cronjob_name, display_name)| {
            let mut owned: Vec<_> = list
                .items
                .iter()
                .filter(|job| job.metadata.owner_references.as_deref().unwrap_or_default().iter().any(|r| r.name == cronjob_name))
                .collect();
            owned.sort_by_key(|job| job.status.as_ref().and_then(|s| s.start_time.as_ref()).map(|t| t.0.as_second()).unwrap_or(0));
            match owned.last() {
                Some(job) => {
                    let status = job.status.as_ref();
                    let (icon, label) = if status.and_then(|s| s.succeeded).unwrap_or(0) > 0 {
                        ("\u{2713}", "complete")
                    } else if status.and_then(|s| s.failed).unwrap_or(0) > 0 {
                        ("\u{2717}", "failed")
                    } else if status.and_then(|s| s.active).unwrap_or(0) > 0 {
                        ("\u{25cf}", "running")
                    } else {
                        ("?", "unknown")
                    };
                    json!({ "name": display_name, "status_icon": icon, "status_label": label })
                }
                None => json!({ "name": display_name, "status_icon": "?", "status_label": "no runs found" }),
            }
        })
        .collect()
}

// ── Postgres-backed: network insights, spike config, claude audit log ──────

async fn fetch_network_insights(pool: &PgPool) -> Vec<Value> {
    let rows = sqlx::query(
        "SELECT id, occurred_at, spike_tx_mbps, baseline_tx_mbps, top_pods, explanation FROM network_insights ORDER BY occurred_at DESC LIMIT 5",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            let occurred_at: chrono::DateTime<chrono::Utc> = r.try_get("occurred_at").unwrap_or_else(|_| chrono::Utc::now());
            json!({
                "id": r.try_get::<i64, _>("id").unwrap_or(0),
                "occurred_at": occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                "spike_tx_mbps": r.try_get::<f64, _>("spike_tx_mbps").unwrap_or(0.0),
                "baseline_tx_mbps": r.try_get::<f64, _>("baseline_tx_mbps").unwrap_or(0.0),
                "top_pods": r.try_get::<Value, _>("top_pods").unwrap_or(Value::Array(vec![])),
                "explanation": r.try_get::<String, _>("explanation").unwrap_or_default(),
            })
        })
        .collect()
}

async fn fetch_spike_config(pool: &PgPool) -> Value {
    let row = sqlx::query("SELECT multiplier, floor_mbps FROM spike_detector_config WHERE id = 1")
        .fetch_optional(pool)
        .await;
    match row {
        Ok(Some(r)) => json!({
            "multiplier": r.try_get::<f64, _>("multiplier").unwrap_or(3.0),
            "floor_mbps": r.try_get::<f64, _>("floor_mbps").unwrap_or(5.0),
        }),
        _ => json!({ "multiplier": 3.0, "floor_mbps": 5.0 }),
    }
}

async fn fetch_claude_audit_log(pool: &PgPool) -> Vec<Value> {
    let rows = sqlx::query(
        "SELECT id, occurred_at, context, model, prompt, response, input_tokens, output_tokens, error \
         FROM claude_audit_log ORDER BY occurred_at DESC LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            json!({
                "id": r.try_get::<i64, _>("id").unwrap_or(0),
                "occurred_at": r.try_get::<String, _>("occurred_at").unwrap_or_default(),
                "context": r.try_get::<String, _>("context").unwrap_or_default(),
                "model": r.try_get::<String, _>("model").unwrap_or_default(),
                "error": r.try_get::<Option<String>, _>("error").ok().flatten(),
            })
        })
        .collect()
}

/// Assembles every cluster panel into one context object for the "cluster"
/// Foster machine. Real work (network I/O to Prometheus/k8s/Postgres);
/// called via `block_in_place`, same pattern as visitors.rs/lighthouse.rs.
/// Latest cargo-audit report — parsed the same way the real
/// `src/components/security_audit.rs` server function does. The report
/// itself is produced by a separate periodic scanner (a small standalone
/// image running cargo-audit against this repo's lockfiles, POSTing the
/// JSON result here with the same Basic-Auth token Lighthouse uses — see
/// security_audit.rs::upload_security_audit and CI milestone 7's
/// security-audit-image-push job), not computed by this process itself.
async fn fetch_security_audit(pool: &PgPool) -> Value {
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT report FROM security_audit ORDER BY uploaded_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(v) = row else {
        return json!({
            "sec_scanned_at": "",
            "sec_dependency_count": 0,
            "sec_status_label": "No audit report yet",
            "sec_advisory_count": 0,
            "security_vulnerabilities": [],
            "security_warnings": [],
        });
    };

    let scanned_at = v["scanned_at"].as_str().unwrap_or("unknown").to_string();
    let dependency_count = v["lockfile"]["dependency-count"].as_u64().unwrap_or(0);

    let to_advisory = |entry: &Value| -> Value {
        let a = &entry["advisory"];
        json!({
            "id": a["id"].as_str().unwrap_or(""),
            "package": a["package"].as_str().unwrap_or(""),
            "title": a["title"].as_str().unwrap_or(""),
            "date": a["date"].as_str().unwrap_or(""),
            "url": a["url"].as_str().unwrap_or(""),
        })
    };

    let vulnerabilities: Vec<Value> = v["vulnerabilities"]["list"]
        .as_array()
        .map(|a| a.iter().map(to_advisory).collect())
        .unwrap_or_default();

    let mut warnings: Vec<Value> = Vec::new();
    if let Some(warn_map) = v["warnings"].as_object() {
        for entries in warn_map.values() {
            for entry in entries.as_array().unwrap_or(&vec![]) {
                warnings.push(to_advisory(entry));
            }
        }
    }

    let status_label = if vulnerabilities.is_empty() { "\u{2713} Clean" } else { "\u{2717} Vulnerabilities found" };

    json!({
        "sec_scanned_at": scanned_at,
        "sec_dependency_count": dependency_count,
        "sec_status_label": status_label,
        "sec_advisory_count": vulnerabilities.len() + warnings.len(),
        "security_vulnerabilities": vulnerabilities,
        "security_warnings": warnings,
    })
}

pub fn fetch_cluster_data(pool: &PgPool) -> Value {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let kube_client = Client::try_default().await.ok();

            let (metrics, nodes, historical, ceph, top_pods, cloudflared) = tokio::join!(
                fetch_cluster_metrics(),
                fetch_node_metrics(),
                fetch_historical_metrics(),
                fetch_ceph_status(),
                fetch_top_network_pods(),
                fetch_cloudflared_status(),
            );
            let (historical_series, historical_summary) = historical;

            let (gitops, backups) = match &kube_client {
                Some(c) => (fetch_gitops_status(c).await, fetch_backup_status(c).await),
                None => (vec![], vec![]),
            };

            let (insights, spike_config, claude_log, security_audit) = tokio::join!(
                fetch_network_insights(pool),
                fetch_spike_config(pool),
                fetch_claude_audit_log(pool),
                fetch_security_audit(pool),
            );

            // fx-text/fx-if/fx-bind-attr only look up a single top-level
            // ctx[key] (no dotted paths), so every scalar we want to bind
            // declaratively has to live at the context root — hence the
            // flattening (via serde_json::Value::Object merge) below,
            // rather than nesting each panel under its own key.
            let mut ctx = serde_json::Map::new();
            for (obj, prefix) in [(&metrics, ""), (&ceph, "ceph_"), (&cloudflared, "cf_"), (&historical_summary, "")] {
                if let Some(map) = obj.as_object() {
                    for (k, v) in map {
                        ctx.insert(format!("{prefix}{k}"), v.clone());
                    }
                }
            }
            if let Some(map) = spike_config.as_object() {
                for (k, v) in map {
                    ctx.insert(format!("spike_{k}"), v.clone());
                }
            }
            ctx.insert("nodes".to_string(), json!(nodes));
            ctx.insert("top_pods".to_string(), json!(top_pods));
            ctx.insert("flux".to_string(), json!(gitops));
            ctx.insert("backups".to_string(), json!(backups));
            ctx.insert("network_insights".to_string(), json!(insights));
            ctx.insert("claude_log".to_string(), json!(claude_log));
            if let Some(map) = security_audit.as_object() {
                for (k, v) in map {
                    ctx.insert(k.clone(), v.clone());
                }
            }
            // Wrapped in a one-element array so it can ride the same
            // fx-for + data-fx-item JS-reading pattern satellites.js uses
            // for structured (non-scalar) data — fx-for expects an array.
            ctx.insert("historical_series_list".to_string(), json!([historical_series]));
            ctx.insert("kube_connected".to_string(), json!(kube_client.is_some()));

            Value::Object(ctx)
        })
    })
}
