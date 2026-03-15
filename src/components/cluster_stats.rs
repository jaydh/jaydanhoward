#![allow(clippy::all)]
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub cpu_usage_percent: f64,
    pub cpu_total_cores: f64,
    pub memory_usage_gb: f64,
    pub memory_total_gb: f64,
    pub disk_usage_gb: f64,
    pub disk_total_gb: f64,
    pub pod_count: u32,
    pub node_count: u32,
    pub healthy_node_count: u32,
    pub network_rx_mbps: f64,
    pub network_tx_mbps: f64,
    pub db_info: Option<DbInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbEntry {
    pub name: String,
    pub size_bytes: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbInfo {
    pub databases: Vec<DbEntry>,
    pub pvc_capacity_bytes: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeMetric {
    pub name: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_gb: f64,
    pub memory_total_gb: f64,
}

#[cfg(feature = "ssr")]
async fn parse_prometheus_value(query: &str) -> Result<f64, ServerFnError<String>> {
    use crate::prometheus_client::query_prometheus;

    let data = query_prometheus(query).await.map_err(|e| {
        ServerFnError::ServerError(format!("Prometheus query failed: {}", e))
    })?;

    if let Some(metric) = data.data.result.first() {
        metric.value.1.parse::<f64>().map_err(|e| {
            ServerFnError::ServerError(format!("Failed to parse value: {}", e))
        })
    } else {
        Ok(0.0)
    }
}

#[server(name = GetClusterMetrics, prefix = "/api", endpoint = "get_cluster_metrics")]
pub async fn get_cluster_metrics() -> Result<ClusterMetrics, ServerFnError<String>> {

    // Cluster CPU metrics
    let cpu_used = parse_prometheus_value(
        "sum(rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))"
    ).await?;
    let cpu_total = parse_prometheus_value(
        "sum(machine_cpu_cores)"
    ).await?;

    // Memory metrics (convert bytes to GB)
    let memory_used = parse_prometheus_value(
        "sum(container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024"
    ).await?;
    let memory_total = parse_prometheus_value(
        "sum(machine_memory_bytes) / 1024 / 1024 / 1024"
    ).await?;

    // Disk metrics from Rook Ceph (convert bytes to GB)
    let disk_used = parse_prometheus_value(
        "ceph_cluster_total_used_bytes / 1024 / 1024 / 1024"
    ).await?;
    let disk_total = parse_prometheus_value(
        "ceph_cluster_total_bytes / 1024 / 1024 / 1024"
    ).await?;

    // Pod count
    let pod_count = parse_prometheus_value(
        "count(kube_pod_info{pod!=\"\"})"
    ).await? as u32;

    // Node count
    let node_count = parse_prometheus_value(
        "count(kube_node_info)"
    ).await? as u32;

    // Healthy node count (nodes in Ready state)
    let healthy_node_count = parse_prometheus_value(
        "sum(kube_node_status_condition{condition=\"Ready\",status=\"true\"})"
    ).await? as u32;

    // Network metrics (convert bytes/sec to Mbps)
    let network_rx = parse_prometheus_value(
        "sum(rate(container_network_receive_bytes_total[5m])) * 8 / 1000 / 1000"
    ).await?;
    let network_tx = parse_prometheus_value(
        "sum(rate(container_network_transmit_bytes_total[5m])) * 8 / 1000 / 1000"
    ).await?;

    let db_info = {
        use crate::prometheus_client::query_prometheus;

        let (db_data, pvc_data) = tokio::join!(
            query_prometheus(
                "sort_desc(pg_database_size_bytes{datname!~\"template.*|postgres\"})"
            ),
            query_prometheus(
                "max(kubelet_volume_stats_capacity_bytes\
                 {namespace=\"service\",persistentvolumeclaim=~\"pgdata-jaydanhoward-postgres-.*\"})"
            ),
        );

        let databases: Vec<DbEntry> = db_data
            .unwrap_or_else(|_| crate::prometheus_client::PrometheusData {
                status: String::new(),
                data: crate::prometheus_client::PrometheusResult { result_type: String::new(), result: vec![] },
            })
            .data
            .result
            .iter()
            .filter_map(|m| {
                let name = m.metric.get("datname")?.clone();
                let size_bytes = m.value.1.parse::<f64>().ok()? as i64;
                Some(DbEntry { name, size_bytes })
            })
            .collect();

        let pvc_capacity_bytes = pvc_data
            .ok()
            .and_then(|d| d.data.result.first().map(|m| m.value.1.parse::<f64>().unwrap_or(0.0) as i64));

        if databases.is_empty() && pvc_capacity_bytes.is_none() {
            None
        } else {
            Some(DbInfo { databases, pvc_capacity_bytes })
        }
    };

    Ok(ClusterMetrics {
        cpu_usage_percent: (cpu_used / cpu_total * 100.0).min(100.0),
        cpu_total_cores: cpu_total,
        memory_usage_gb: memory_used,
        memory_total_gb: memory_total,
        disk_usage_gb: disk_used,
        disk_total_gb: disk_total,
        pod_count,
        node_count,
        healthy_node_count,
        network_rx_mbps: network_rx,
        network_tx_mbps: network_tx,
        db_info,
    })
}

#[server(name = GetNodeMetrics, prefix = "/api", endpoint = "get_node_metrics")]
pub async fn get_node_metrics() -> Result<Vec<NodeMetric>, ServerFnError<String>> {
    use crate::prometheus_client::query_prometheus;

    // Get all nodes with their metrics
    let cpu_data = query_prometheus(
        "sum by (node) (rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))"
    ).await.map_err(|e| {
        ServerFnError::ServerError(format!("Failed to get node CPU metrics: {}", e))
    })?;

    let memory_data = query_prometheus(
        "sum by (node) (container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024"
    ).await.map_err(|e| {
        ServerFnError::ServerError(format!("Failed to get node memory metrics: {}", e))
    })?;

    let memory_total_data = query_prometheus(
        "sum by (node) (machine_memory_bytes) / 1024 / 1024 / 1024"
    ).await.map_err(|e| {
        ServerFnError::ServerError(format!("Failed to get node memory total: {}", e))
    })?;

    let cpu_capacity_data = query_prometheus(
        "sum by (node) (machine_cpu_cores)"
    ).await.map_err(|e| {
        ServerFnError::ServerError(format!("Failed to get node CPU capacity: {}", e))
    })?;

    // Parse and combine metrics
    let mut nodes = std::collections::HashMap::new();

    for metric in cpu_data.data.result {
        if let Some(node) = metric.metric.get("node") {
            let cpu_used: f64 = metric.value.1.parse().unwrap_or(0.0);
            nodes.entry(node.clone()).or_insert(NodeMetric {
                name: node.clone(),
                cpu_usage_percent: 0.0,
                memory_usage_gb: 0.0,
                memory_total_gb: 0.0,
            }).cpu_usage_percent = cpu_used;
        }
    }

    for metric in memory_data.data.result {
        if let Some(node) = metric.metric.get("node") {
            let mem_used: f64 = metric.value.1.parse().unwrap_or(0.0);
            if let Some(node_metric) = nodes.get_mut(node) {
                node_metric.memory_usage_gb = mem_used;
            }
        }
    }

    for metric in memory_total_data.data.result {
        if let Some(node) = metric.metric.get("node") {
            let mem_total: f64 = metric.value.1.parse().unwrap_or(0.0);
            if let Some(node_metric) = nodes.get_mut(node) {
                node_metric.memory_total_gb = mem_total;
            }
        }
    }

    // Calculate CPU percentage
    for metric in cpu_capacity_data.data.result {
        if let Some(node) = metric.metric.get("node") {
            let cpu_total: f64 = metric.value.1.parse().unwrap_or(1.0);
            if let Some(node_metric) = nodes.get_mut(node) {
                node_metric.cpu_usage_percent =
                    (node_metric.cpu_usage_percent / cpu_total * 100.0).min(100.0);
            }
        }
    }

    Ok(nodes.into_values().collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoricalMetrics {
    pub cpu_history: Vec<f64>,
    pub memory_history: Vec<f64>,
    pub disk_history: Vec<f64>,
    pub network_rx_history: Vec<f64>,
    pub network_tx_history: Vec<f64>,
}

#[server(name = GetHistoricalMetrics, prefix = "/api", endpoint = "get_historical_metrics")]
pub async fn get_historical_metrics() -> Result<HistoricalMetrics, ServerFnError<String>> {
    use crate::prometheus_client::query_prometheus_range;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ServerFnError::ServerError(format!("Time error: {}", e)))?
        .as_secs() as i64;

    let twenty_four_hours_ago = now - (24 * 3600);
    let step = "10m"; // 10 minute intervals for 24 hours = 144 points

    // Query historical CPU usage
    let cpu_data = query_prometheus_range(
        "sum(rate(container_cpu_usage_seconds_total{container!=\"\"}[5m])) / sum(machine_cpu_cores) * 100",
        twenty_four_hours_ago,
        now,
        step,
    ).await.map_err(|e| ServerFnError::ServerError(format!("CPU query failed: {}", e)))?;

    // Query historical memory usage
    let memory_data = query_prometheus_range(
        "sum(container_memory_working_set_bytes{container!=\"\"}) / sum(machine_memory_bytes) * 100",
        twenty_four_hours_ago,
        now,
        step,
    ).await.map_err(|e| ServerFnError::ServerError(format!("Memory query failed: {}", e)))?;

    // Query historical disk usage from Rook Ceph
    let disk_data = query_prometheus_range(
        "ceph_cluster_total_used_bytes / ceph_cluster_total_bytes * 100",
        twenty_four_hours_ago,
        now,
        step,
    ).await.map_err(|e| ServerFnError::ServerError(format!("Disk query failed: {}", e)))?;

    // Query historical network RX
    let network_rx_data = query_prometheus_range(
        "sum(rate(container_network_receive_bytes_total[5m])) * 8 / 1000 / 1000",
        twenty_four_hours_ago,
        now,
        step,
    ).await.map_err(|e| ServerFnError::ServerError(format!("Network RX query failed: {}", e)))?;

    // Query historical network TX
    let network_tx_data = query_prometheus_range(
        "sum(rate(container_network_transmit_bytes_total[5m])) * 8 / 1000 / 1000",
        twenty_four_hours_ago,
        now,
        step,
    ).await.map_err(|e| ServerFnError::ServerError(format!("Network TX query failed: {}", e)))?;

    // Extract values from responses
    let cpu_history: Vec<f64> = cpu_data.data.result.first()
        .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    let memory_history: Vec<f64> = memory_data.data.result.first()
        .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    let disk_history: Vec<f64> = disk_data.data.result.first()
        .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    let network_rx_history: Vec<f64> = network_rx_data.data.result.first()
        .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    let network_tx_history: Vec<f64> = network_tx_data.data.result.first()
        .map(|m| m.values.iter().map(|(_, v)| v.parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    Ok(HistoricalMetrics {
        cpu_history,
        memory_history,
        disk_history,
        network_rx_history,
        network_tx_history,
    })
}

#[component]
fn NodeCard(node: NodeMetric) -> impl IntoView {
    let mem_pct = (node.memory_usage_gb / node.memory_total_gb * 100.0).min(100.0);

    view! {
        <div class="bg-surface rounded-lg shadow-sm p-3 border border-border">
            <h4 class="font-medium text-sm text-charcoal mb-2">{node.name}</h4>
            <div class="space-y-1.5 text-xs">
                <div class="flex justify-between items-center">
                    <span class="text-charcoal-lighter">"CPU"</span>
                    <span class="font-semibold">{format!("{:.1}%", node.cpu_usage_percent)}</span>
                </div>
                <div class="w-full bg-border rounded-full h-1">
                    <div
                        class="bg-blue-500 h-1 rounded-full transition-all"
                        style={format!("width: {}%", node.cpu_usage_percent)}
                    ></div>
                </div>
                <div class="flex justify-between items-center">
                    <span class="text-charcoal-lighter">"Mem"</span>
                    <span class="font-semibold">{format!("{:.1}G", node.memory_usage_gb)}</span>
                </div>
                <div class="w-full bg-border rounded-full h-1">
                    <div
                        class="bg-purple-500 h-1 rounded-full transition-all"
                        style={format!("width: {}%", mem_pct)}
                    ></div>
                </div>
            </div>
        </div>
    }
}

fn fmt_db_size(bytes: i64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else {
        format!("{:.0}M", bytes as f64 / 1_048_576.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CephStatus {
    pub health: u32, // 0=OK, 1=WARN, 2=ERR
    // Services
    pub mon_quorum: u32,
    pub mon_total: u32,
    pub mgr_active: u32,
    pub mgr_standby: u32,
    pub mds_up: u32,
    pub mds_standby: u32,
    pub osd_up: u32,
    pub osd_in: u32,
    pub osd_total: u32,
    pub rgw_count: u32,
    // Data
    pub volumes_healthy: u32,
    pub volumes_total: u32,
    pub pool_count: u32,
    pub pg_total: u32,
    pub pg_clean: u32,
    pub pg_degraded: u32,
    pub pg_recovering: u32,
    pub pg_remapped: u32,
    pub pg_scrubbing: u32,
    pub pg_deep_scrub: u32,
    pub objects_count: f64,
    pub data_used_bytes: f64,
    pub data_avail_bytes: f64,
    pub data_total_bytes: f64,
    // IO (raw bytes/sec)
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub read_iops: f64,
    pub write_iops: f64,
}

#[server(name = GetCephStatus, prefix = "/api", endpoint = "get_ceph_status")]
pub async fn get_ceph_status() -> Result<CephStatus, ServerFnError<String>> {
    let health = parse_prometheus_value("ceph_health_status").await.unwrap_or(0.0) as u32;

    // MON
    let mon_quorum = parse_prometheus_value("sum(ceph_mon_quorum_status == 1)").await.unwrap_or(0.0) as u32;
    let mon_total = parse_prometheus_value("count(ceph_mon_quorum_status)").await.unwrap_or(0.0) as u32;

    // MGR
    let mgr_active = parse_prometheus_value("sum(ceph_mgr_status == 1)").await.unwrap_or(0.0) as u32;
    let mgr_standby = parse_prometheus_value("sum(ceph_mgr_status == 0)").await.unwrap_or(0.0) as u32;

    // MDS
    let mds_up = parse_prometheus_value("count(ceph_mds_metadata{fs_state=\"up:active\"})").await.unwrap_or(0.0) as u32;
    let mds_standby = parse_prometheus_value("count(ceph_mds_metadata{fs_state=~\"up:standby.*\"})").await.unwrap_or(0.0) as u32;

    // OSD
    let osd_up = parse_prometheus_value("count(ceph_osd_up == 1)").await.unwrap_or(0.0) as u32;
    let osd_in = parse_prometheus_value("count(ceph_osd_in == 1)").await.unwrap_or(0.0) as u32;
    let osd_total = parse_prometheus_value("count(ceph_osd_up)").await.unwrap_or(0.0) as u32;

    // RGW
    let rgw_count = parse_prometheus_value("count(ceph_rgw_metadata)").await.unwrap_or(0.0) as u32;

    // Volumes (CephFS)
    let volumes_total = parse_prometheus_value("count(ceph_fs_metadata)").await.unwrap_or(0.0) as u32;
    let volumes_healthy = volumes_total; // assume healthy if metric is present

    // Pools and PGs — use osdmap metrics for accurate counts
    let pool_count = parse_prometheus_value("ceph_osdmap_num_pools").await.unwrap_or(0.0) as u32;
    let pg_total = parse_prometheus_value("ceph_osdmap_num_pg").await.unwrap_or(0.0) as u32;
    let pg_clean = parse_prometheus_value("ceph_pg_state{state=\"active+clean\"}").await.unwrap_or(0.0) as u32;
    let pg_degraded = parse_prometheus_value("sum(ceph_pg_state{state=~\".*degraded.*\"})").await.unwrap_or(0.0) as u32;
    let pg_recovering = parse_prometheus_value("sum(ceph_pg_state{state=~\".*recovering.*\"})").await.unwrap_or(0.0) as u32;
    let pg_remapped = parse_prometheus_value("sum(ceph_pg_state{state=~\".*remapped.*\"})").await.unwrap_or(0.0) as u32;
    let pg_scrubbing = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing(?!\\\\+deep).*\"})").await.unwrap_or(0.0) as u32;
    let pg_deep_scrub = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing\\\\+deep.*\"})").await.unwrap_or(0.0) as u32;

    // Objects
    let objects_count = parse_prometheus_value("sum(ceph_pool_objects_total)").await.unwrap_or(0.0);

    // Data usage (raw bytes)
    let data_used_bytes = parse_prometheus_value("ceph_cluster_total_used_bytes").await.unwrap_or(0.0);
    let data_total_bytes = parse_prometheus_value("ceph_cluster_total_bytes").await.unwrap_or(0.0);
    let data_avail_bytes = (data_total_bytes - data_used_bytes).max(0.0);

    // IO — raw bytes/sec and iops
    let read_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_r_out_bytes[5m]))").await.unwrap_or(0.0);
    let write_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_w_in_bytes[5m]))").await.unwrap_or(0.0);
    let read_iops = parse_prometheus_value("sum(irate(ceph_osd_op_r[5m]))").await.unwrap_or(0.0);
    let write_iops = parse_prometheus_value("sum(irate(ceph_osd_op_w[5m]))").await.unwrap_or(0.0);

    Ok(CephStatus {
        health,
        mon_quorum, mon_total,
        mgr_active, mgr_standby,
        mds_up, mds_standby,
        osd_up, osd_in, osd_total,
        rgw_count,
        volumes_healthy, volumes_total,
        pool_count, pg_total, pg_clean,
        pg_degraded, pg_recovering, pg_remapped, pg_scrubbing, pg_deep_scrub,
        objects_count,
        data_used_bytes, data_avail_bytes, data_total_bytes,
        read_bytes_per_sec, write_bytes_per_sec, read_iops, write_iops,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodTraffic {
    pub namespace: String,
    pub pod: String,
    pub mbps: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkInsight {
    pub id: i64,
    pub occurred_at: String,
    pub spike_tx_mbps: f64,
    pub baseline_tx_mbps: f64,
    pub top_pods: Vec<PodTraffic>,
    pub explanation: String,
}

#[server(name = GetNetworkInsights, prefix = "/api", endpoint = "get_network_insights")]
pub async fn get_network_insights() -> Result<Vec<NetworkInsight>, ServerFnError<String>> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    let pool = extract::<Data<PgPool>>().await
        .map_err(|_| ServerFnError::ServerError("no db".into()))?;

    let rows = crate::db::get_recent_network_insights(&pool, 5)
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(rows.into_iter().map(|r| NetworkInsight {
        id: r.id,
        occurred_at: r.occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        spike_tx_mbps: r.spike_tx_mbps,
        baseline_tx_mbps: r.baseline_tx_mbps,
        top_pods: serde_json::from_value(r.top_pods).unwrap_or_default(),
        explanation: r.explanation,
    }).collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluxResource {
    pub kind: String,
    pub namespace: String,
    pub name: String,
    pub ready: bool,
}

#[server(name = GetGitOpsStatus, prefix = "/api", endpoint = "get_gitops_status")]
pub async fn get_gitops_status() -> Result<Vec<FluxResource>, ServerFnError<String>> {
    use crate::prometheus_client::query_prometheus;

    // gotk_reconcile_condition{type="Ready"} has value 1 per resource+status combo.
    // Labels: kind, name, exported_namespace (or namespace), status ("True"/"False"/"Unknown").
    let data = query_prometheus("gotk_reconcile_condition{type=\"Ready\"}")
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let mut resources: Vec<FluxResource> = data
        .data
        .result
        .iter()
        .map(|m| {
            let kind = m.metric.get("kind").cloned().unwrap_or_default();
            let name = m.metric.get("name").cloned().unwrap_or_default();
            let namespace = m.metric
                .get("exported_namespace")
                .or_else(|| m.metric.get("namespace"))
                .cloned()
                .unwrap_or_default();
            let status = m.metric.get("status").map(|s| s.as_str()).unwrap_or("");
            FluxResource { kind, namespace, name, ready: status == "True" }
        })
        .filter(|r| !r.name.is_empty())
        .collect();

    // Sort: failing first, then alphabetically by kind + name.
    resources.sort_by(|a, b| {
        a.ready.cmp(&b.ready)
            .then(a.kind.cmp(&b.kind))
            .then(a.name.cmp(&b.name))
    });

    Ok(resources)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaudeAuditEntry {
    pub id: i64,
    pub occurred_at: String,
    pub context: String,
    pub model: String,
    pub prompt: String,
    pub response: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub error: Option<String>,
}

#[server(name = GetClaudeAuditLog, prefix = "/api", endpoint = "get_claude_audit_log")]
pub async fn get_claude_audit_log() -> Result<Vec<ClaudeAuditEntry>, ServerFnError<String>> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    let pool = extract::<Data<PgPool>>().await
        .map_err(|_| ServerFnError::ServerError("no db".into()))?;

    let rows = crate::db::get_recent_claude_audits(&pool, 20)
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ClaudeAuditEntry {
        id: r.id,
        occurred_at: r.occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        context: r.context,
        model: r.model,
        prompt: r.prompt,
        response: r.response,
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        error: r.error,
    }).collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsUpdate {
    pub cluster: Option<ClusterMetrics>,
    pub nodes: Vec<NodeMetric>,
    pub ceph: Option<CephStatus>,
    pub latest_insight: Option<NetworkInsight>,
}

fn fmt_ceph_bytes(bytes: f64) -> String {
    if bytes >= 1_099_511_627_776.0 {
        format!("{:.1} TiB", bytes / 1_099_511_627_776.0)
    } else if bytes >= 1_073_741_824.0 {
        format!("{:.1} GiB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.1} MiB", bytes / 1_048_576.0)
    } else if bytes >= 1_024.0 {
        format!("{:.0} KiB", bytes / 1_024.0)
    } else {
        format!("{:.0} B", bytes)
    }
}

fn fmt_ceph_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", fmt_ceph_bytes(bytes_per_sec))
}

fn fmt_objects(count: f64) -> String {
    if count >= 1_000_000.0 {
        format!("{:.2}M", count / 1_000_000.0)
    } else if count >= 1_000.0 {
        format!("{:.2}k", count / 1_000.0)
    } else {
        format!("{:.0}", count)
    }
}

#[component]
fn NetworkInsightsPanel(insights: Vec<NetworkInsight>) -> impl IntoView {
    view! {
        <div class="bg-surface rounded-lg shadow-sm p-4 border border-border mt-4">
            <h3 class="text-xs font-medium text-charcoal-lighter mb-3">"Network Insights"</h3>
            <div class="space-y-3">
                {insights.into_iter().map(|insight| {
                    let multiplier = insight.spike_tx_mbps / insight.baseline_tx_mbps.max(0.1);
                    view! {
                        <div class="border-l-2 border-amber-400 pl-3">
                            <div class="flex items-baseline gap-2 mb-1">
                                <span class="text-xs font-mono font-semibold text-amber-500">
                                    {format!("↑ {:.0} Mbps", insight.spike_tx_mbps)}
                                </span>
                                <span class="text-xs text-charcoal-lighter">
                                    {format!("{:.1}x baseline", multiplier)}
                                </span>
                                <span class="text-xs text-charcoal-lighter ml-auto">
                                    {insight.occurred_at}
                                </span>
                            </div>
                            <p class="text-xs text-charcoal leading-relaxed">
                                {insight.explanation}
                            </p>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn GitOpsPanel(resources: Vec<FluxResource>) -> impl IntoView {
    // Group by kind.
    let mut groups: Vec<(String, Vec<FluxResource>)> = Vec::new();
    for r in resources {
        if let Some(g) = groups.iter_mut().find(|(k, _)| k == &r.kind) {
            g.1.push(r);
        } else {
            groups.push((r.kind.clone(), vec![r]));
        }
    }

    let total: usize = groups.iter().map(|(_, rs)| rs.len()).sum();
    let failing: usize = groups.iter().flat_map(|(_, rs)| rs).filter(|r| !r.ready).count();

    view! {
        <div class="bg-surface rounded-lg shadow-sm p-4 border border-border">
            <div class="flex items-center gap-3 mb-4">
                <h3 class="text-xs font-medium text-charcoal-lighter">"Flux GitOps"</h3>
                <span class="text-xs text-charcoal-lighter">{total} " resources"</span>
                {(failing > 0).then(|| view! {
                    <span class="text-xs font-medium text-red-500">{failing} " failing"</span>
                })}
                {(failing == 0).then(|| view! {
                    <span class="text-xs text-green-600">"✓ all ready"</span>
                })}
            </div>
            <div class="space-y-4">
                {groups.into_iter().map(|(kind, rs)| {
                    let kind_failing = rs.iter().filter(|r| !r.ready).count();
                    view! {
                        <div>
                            <div class="flex items-center gap-2 mb-1">
                                <span class="text-xs font-medium text-charcoal">{kind}</span>
                                {(kind_failing > 0).then(|| view! {
                                    <span class="text-xs text-red-500">{kind_failing} " not ready"</span>
                                })}
                            </div>
                            <div class="space-y-0.5">
                                {rs.into_iter().map(|r| view! {
                                    <div class="flex items-center gap-2 text-xs font-mono">
                                        <span class=if r.ready { "text-green-600" } else { "text-red-500" }>
                                            {if r.ready { "●" } else { "●" }}
                                        </span>
                                        <span class="text-charcoal-lighter">{r.namespace} "/"</span>
                                        <span class="text-charcoal">{r.name}</span>
                                    </div>
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn ClaudeAuditPanel(entries: Vec<ClaudeAuditEntry>) -> impl IntoView {
    let (expanded, set_expanded) = signal(None::<i64>);

    view! {
        <div class="bg-surface rounded-lg shadow-sm p-4 border border-border mt-4">
            <h3 class="text-xs font-medium text-charcoal-lighter mb-3">"Claude API Audit"</h3>
            <div class="space-y-2">
                {entries.into_iter().map(|entry| {
                    let id = entry.id;
                    let is_error = entry.error.is_some();
                    let tokens = match (entry.input_tokens, entry.output_tokens) {
                        (Some(i), Some(o)) => format!("{i}→{o} tok"),
                        _ => String::new(),
                    };
                    let response_preview = entry.response.as_deref()
                        .map(|r| {
                            // Extract explanation from JSON if present, else show raw
                            serde_json::from_str::<serde_json::Value>(r)
                                .ok()
                                .and_then(|v| v["explanation"].as_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| r.chars().take(120).collect::<String>())
                        })
                        .or_else(|| entry.error.as_deref().map(|e| format!("error: {e}")))
                        .unwrap_or_default();
                    let prompt = entry.prompt.clone();
                    let full_response = entry.response.clone().unwrap_or_default();

                    view! {
                        <div class=move || format!(
                            "border-l-2 pl-3 {}",
                            if is_error { "border-red-500" } else { "border-blue-400" }
                        )>
                            <div class="flex items-center gap-2 text-xs">
                                <span class="font-mono text-charcoal-lighter">{entry.occurred_at}</span>
                                <span class="text-blue-400">{entry.context}</span>
                                <span class="text-charcoal-lighter">{entry.model}</span>
                                {(!tokens.is_empty()).then(|| view! {
                                    <span class="text-charcoal-lighter ml-auto">{tokens}</span>
                                })}
                                <button
                                    class="text-charcoal-lighter hover:text-charcoal ml-1"
                                    on:click=move |_| set_expanded.update(|v| {
                                        *v = if *v == Some(id) { None } else { Some(id) };
                                    })
                                >
                                    {move || if expanded.get() == Some(id) { "▲" } else { "▼" }}
                                </button>
                            </div>
                            <p class="text-xs text-charcoal mt-0.5 leading-relaxed line-clamp-2">
                                {response_preview}
                            </p>
                            {move || (expanded.get() == Some(id)).then(|| view! {
                                <div class="mt-2 space-y-2">
                                    <div>
                                        <p class="text-xs font-medium text-charcoal-lighter mb-1">"Prompt"</p>
                                        <pre class="text-xs text-charcoal bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">
                                            {prompt.clone()}
                                        </pre>
                                    </div>
                                    {(!full_response.is_empty()).then(|| view! {
                                        <div>
                                            <p class="text-xs font-medium text-charcoal-lighter mb-1">"Raw response"</p>
                                            <pre class="text-xs text-charcoal bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-32 overflow-y-auto">
                                                {full_response.clone()}
                                            </pre>
                                        </div>
                                    })}
                                </div>
                            })}
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn CephStatusPanel(ceph: CephStatus) -> impl IntoView {
    let health = ceph.health;
    let mon_quorum = ceph.mon_quorum;
    let mon_total = ceph.mon_total;
    let mgr_active = ceph.mgr_active;
    let mgr_standby = ceph.mgr_standby;
    let mds_up = ceph.mds_up;
    let mds_standby = ceph.mds_standby;
    let osd_up = ceph.osd_up;
    let osd_in = ceph.osd_in;
    let osd_total = ceph.osd_total;
    let rgw_count = ceph.rgw_count;
    let volumes_healthy = ceph.volumes_healthy;
    let volumes_total = ceph.volumes_total;
    let pool_count = ceph.pool_count;
    let pg_total = ceph.pg_total;
    let pg_clean = ceph.pg_clean;
    let pg_degraded = ceph.pg_degraded;
    let pg_recovering = ceph.pg_recovering;
    let pg_remapped = ceph.pg_remapped;
    let pg_scrubbing = ceph.pg_scrubbing;
    let pg_deep_scrub = ceph.pg_deep_scrub;
    let objects_count = ceph.objects_count;
    let data_used_bytes = ceph.data_used_bytes;
    let data_avail_bytes = ceph.data_avail_bytes;
    let data_total_bytes = ceph.data_total_bytes;
    let read_bytes_per_sec = ceph.read_bytes_per_sec;
    let write_bytes_per_sec = ceph.write_bytes_per_sec;
    let read_iops = ceph.read_iops;
    let write_iops = ceph.write_iops;

    let (health_label, health_class) = match health {
        0 => ("HEALTH_OK", "text-green-500"),
        1 => ("HEALTH_WARN", "text-yellow-500"),
        _ => ("HEALTH_ERR", "text-red-500"),
    };

    view! {
        <div class="bg-surface rounded-lg shadow-sm p-4 border border-border mt-4">
            <div class="flex items-center justify-between mb-3">
                <h3 class="text-xs font-medium text-charcoal-lighter">"Ceph"</h3>
                <span class={"text-xs font-mono font-semibold ".to_string() + health_class}>{health_label}</span>
            </div>
            <div class="font-mono text-xs text-charcoal">
                // Services section
                <div class="mb-2">
                    <div class="text-charcoal-lighter mb-1">"services:"</div>
                    <div class="space-y-0.5 ml-2">
                        <div>
                            <span class="text-charcoal-lighter inline-block w-5">"mon:"</span>
                            " "
                            {format!("{} daemons, quorum ({}/{})", mon_total, mon_quorum, mon_total)}
                        </div>
                        {(mgr_active > 0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter inline-block w-5">"mgr:"</span>
                                " "
                                {format!("{} active, {} standby", mgr_active, mgr_standby)}
                            </div>
                        })}
                        {(mds_up > 0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter inline-block w-5">"mds:"</span>
                                " "
                                {format!("{}/{} daemons up, {} hot standby", mds_up, mds_up, mds_standby)}
                            </div>
                        })}
                        <div>
                            <span class="text-charcoal-lighter inline-block w-5">"osd:"</span>
                            " "
                            {format!("{} osds: {} up, {} in", osd_total, osd_up, osd_in)}
                        </div>
                        {(rgw_count > 0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter inline-block w-5">"rgw:"</span>
                                " "
                                {format!("{} daemon{} active", rgw_count, if rgw_count == 1 { "" } else { "s" })}
                            </div>
                        })}
                    </div>
                </div>

                // Data section
                <div class="mb-2">
                    <div class="text-charcoal-lighter mb-1">"data:"</div>
                    <div class="space-y-0.5 ml-2">
                        {(volumes_total > 0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter">"volumes: "</span>
                                {format!("{}/{} healthy", volumes_healthy, volumes_total)}
                            </div>
                        })}
                        {(pool_count > 0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter">"pools:   "</span>
                                {format!("{} pools, {} pgs", pool_count, pg_total)}
                            </div>
                        })}
                        {(objects_count > 0.0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter">"objects: "</span>
                                {format!("{} objects, {}", fmt_objects(objects_count), fmt_ceph_bytes(data_used_bytes))}
                            </div>
                        })}
                        {(data_total_bytes > 0.0).then(move || view! {
                            <div>
                                <span class="text-charcoal-lighter">"usage:   "</span>
                                {format!("{} used, {} / {} avail",
                                    fmt_ceph_bytes(data_used_bytes),
                                    fmt_ceph_bytes(data_avail_bytes),
                                    fmt_ceph_bytes(data_total_bytes))}
                            </div>
                        })}
                        {(pg_total > 0).then(move || {
                            let non_clean = vec![
                                (pg_clean, "active+clean"),
                                (pg_degraded, "degraded"),
                                (pg_recovering, "recovering"),
                                (pg_remapped, "active+clean+remapped"),
                                (pg_scrubbing, "active+clean+scrubbing"),
                                (pg_deep_scrub, "active+clean+scrubbing+deep"),
                            ];
                            let lines: Vec<_> = non_clean.into_iter()
                                .filter(|(n, _)| *n > 0)
                                .collect();
                            view! {
                                <div>
                                    <span class="text-charcoal-lighter">"pgs:     "</span>
                                    <span class="inline-block">
                                        {lines.into_iter().enumerate().map(|(i, (n, state))| {
                                            let prefix = if i == 0 { String::new() } else { "         ".to_string() };
                                            view! {
                                                <span class="block">{format!("{}{} {}", prefix, n, state)}</span>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </span>
                                </div>
                            }
                        })}
                    </div>
                </div>

                // IO section
                {(read_bytes_per_sec > 0.0 || write_bytes_per_sec > 0.0 || read_iops > 0.0 || write_iops > 0.0).then(move || view! {
                    <div>
                        <div class="text-charcoal-lighter mb-1">"io:"</div>
                        <div class="ml-2">
                            <span class="text-charcoal-lighter">"client:  "</span>
                            {format!("{} rd, {} wr, {:.0} op/s rd, {:.0} op/s wr",
                                fmt_ceph_rate(read_bytes_per_sec),
                                fmt_ceph_rate(write_bytes_per_sec),
                                read_iops, write_iops)}
                        </div>
                    </div>
                })}
            </div>
        </div>
    }
}

#[component]
pub fn ClusterStats() -> impl IntoView {
    use std::collections::VecDeque;

    let (cluster_metrics, set_cluster_metrics) = signal(None::<ClusterMetrics>);
    let (node_metrics, set_node_metrics) = signal(Vec::<NodeMetric>::new());
    #[allow(unused_variables)]
    let (ceph_status, set_ceph_status) = signal(None::<CephStatus>);
    #[allow(unused_variables)]
    let (network_insights, set_network_insights) = signal(Vec::<NetworkInsight>::new());
    #[allow(unused_variables)]
    let (audit_log, set_audit_log) = signal(Vec::<ClaudeAuditEntry>::new());
    #[allow(unused_variables)]
    let (gitops, set_gitops) = signal(Vec::<FluxResource>::new());

    // Note: set_last_refresh is used in the WASM-only closure below,
    // but Rust can't see through the .forget() pattern
    #[allow(unused_variables)]
    let (last_refresh, set_last_refresh) = signal(None::<String>);

    // Historical data for charts
    // Dual-window approach: 144 historical points (24 hours at 10-min intervals) + rolling real-time updates
    // When live metrics arrive, they append to history; old points are dropped once we exceed capacity

    // Note: setters are used in closures below, but Rust can't see through the .forget() pattern
    #[allow(unused_variables)]
    let (cpu_history, set_cpu_history) = signal(VecDeque::<f64>::with_capacity(144));
    #[allow(unused_variables)]
    let (memory_history, set_memory_history) = signal(VecDeque::<f64>::with_capacity(144));
    #[allow(unused_variables)]
    let (disk_history, set_disk_history) = signal(VecDeque::<f64>::with_capacity(144));
    #[allow(unused_variables)]
    let (network_rx_history, set_network_rx_history) = signal(VecDeque::<f64>::with_capacity(144));
    #[allow(unused_variables)]
    let (network_tx_history, set_network_tx_history) = signal(VecDeque::<f64>::with_capacity(144));

    // Note: set_error is used in closures below, but Rust can't see through
    // the .forget() pattern required for WASM event handlers
    #[allow(unused_variables)]
    let (error, set_error) = signal(None::<String>);

    // Helper macro to update history with max capacity
    macro_rules! update_history {
        ($setter:expr, $value:expr) => {
            $setter.update(|h| {
                if h.len() >= 144 { h.pop_front(); }
                h.push_back($value);
            });
        };
        ($setter:expr, $values:expr, init) => {
            $setter.update(|h| {
                *h = $values.into_iter().collect();
            });
        };
    }

    // Load recent insights and audit log on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            if let Ok(insights) = get_network_insights().await {
                set_network_insights.set(insights);
            }
            if let Ok(entries) = get_claude_audit_log().await {
                set_audit_log.set(entries);
            }
            if let Ok(resources) = get_gitops_status().await {
                set_gitops.set(resources);
            }
        });
    });

    // Fetch historical data on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match get_historical_metrics().await {
                Ok(historical) => {
                    update_history!(set_cpu_history, historical.cpu_history, init);
                    update_history!(set_memory_history, historical.memory_history, init);
                    update_history!(set_disk_history, historical.disk_history, init);
                    update_history!(set_network_rx_history, historical.network_rx_history, init);
                    update_history!(set_network_tx_history, historical.network_tx_history, init);
                }
                Err(e) => {
                    #[cfg(feature = "ssr")]
                    tracing::error!("Failed to fetch historical metrics: {:?}", e);
                    #[cfg(not(feature = "ssr"))]
                    web_sys::console::error_1(&format!("Failed to fetch historical metrics: {:?}", e).into());
                }
            }
        });
    });

    // Set up SSE connection on client side only
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        use web_sys::{EventSource, MessageEvent};

        let event_source = EventSource::new("/api/metrics/stream").ok();

        if let Some(es) = event_source.clone() {
            let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Some(data) = e.data().as_string() {
                    match serde_json::from_str::<MetricsUpdate>(&data) {
                        Ok(update) => {
                            if let Some(cluster) = update.cluster.clone() {
                                // Update current metrics
                                set_cluster_metrics.set(Some(cluster.clone()));

                                // Update last refresh time
                                #[cfg(not(feature = "ssr"))]
                                {
                                    let now = js_sys::Date::new_0();
                                    let time_str = format!("{:02}:{:02}:{:02}",
                                        now.get_hours(),
                                        now.get_minutes(),
                                        now.get_seconds()
                                    );
                                    set_last_refresh.set(Some(time_str));
                                }

                                // Update historical data (store every data point, limit to 144 points total)
                                update_history!(set_cpu_history, cluster.cpu_usage_percent);
                                update_history!(set_memory_history, (cluster.memory_usage_gb / cluster.memory_total_gb * 100.0).min(100.0));
                                update_history!(set_disk_history, (cluster.disk_usage_gb / cluster.disk_total_gb * 100.0).min(100.0));
                                update_history!(set_network_rx_history, cluster.network_rx_mbps);
                                update_history!(set_network_tx_history, cluster.network_tx_mbps);
                            }
                            set_node_metrics.set(update.nodes);
                            set_ceph_status.set(update.ceph);
                            if let Some(insight) = update.latest_insight {
                                set_network_insights.update(|v| {
                                    if v.iter().all(|i| i.id != insight.id) {
                                        v.insert(0, insight);
                                        v.truncate(5);
                                    }
                                });
                            }
                            set_error.set(None);
                        }
                        Err(e) => {
                            set_error.set(Some(format!("Failed to parse metrics: {}", e)));
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>);

            es.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));

            let onerror_callback = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                set_error.set(Some("Connection lost, reconnecting...".to_string()));
            }) as Box<dyn FnMut(web_sys::Event)>);

            es.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));

            // Store closures to prevent them from being dropped
            // They need to live for the lifetime of the component
            onmessage_callback.forget();
            onerror_callback.forget();

            // Store EventSource using StoredValue::new_local for non-Send values
            let stored_source = StoredValue::new_local(event_source);

            // Clean up EventSource on component unmount
            on_cleanup(move || {
                stored_source.update_value(|source| {
                    if let Some(es) = source {
                        let _ = es.close();
                    }
                });
            });
        } else {
            set_error.set(Some("Failed to create EventSource".to_string()));
        }
    });

    // Fallback: fetch initial data on SSR
    #[cfg(feature = "ssr")]
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            if let Ok(metrics) = get_cluster_metrics().await {
                set_cluster_metrics.set(Some(metrics));
            }
            if let Ok(nodes) = get_node_metrics().await {
                set_node_metrics.set(nodes);
            }
            if let Ok(ceph) = get_ceph_status().await {
                set_ceph_status.set(Some(ceph));
            }
            if let Ok(insights) = get_network_insights().await {
                set_network_insights.set(insights);
            }
            if let Ok(entries) = get_claude_audit_log().await {
                set_audit_log.set(entries);
            }
            if let Ok(resources) = get_gitops_status().await {
                set_gitops.set(resources);
            }
        });
    });

    let (active_tab, set_active_tab) = signal("overview");

    let tab_class = move |name: &'static str| {
        if active_tab.get() == name {
            "px-4 py-2 text-sm font-medium border-b-2 border-accent text-charcoal"
        } else {
            "px-4 py-2 text-sm text-charcoal-lighter hover:text-charcoal border-b-2 border-transparent"
        }
    };

    view! {
        <div class="w-full bg-gray py-6 px-4 rounded-xl mb-8">

            // ── Header ────────────────────────────────────────────────────────
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-bold text-charcoal">"Homelab Cluster"</h2>
                <div class="text-right">
                    <span class="text-xs text-green-600">"● Live"</span>
                    {move || last_refresh.get().map(|t| view! {
                        <span class="text-xs text-charcoal-lighter ml-2">{t}</span>
                    })}
                </div>
            </div>

            {move || error.get().map(|err| view! {
                <div class="text-center text-red-500 p-4 mb-4">
                    <p>"Connection error: " {err}</p>
                </div>
            })}


            // ── Tab bar ───────────────────────────────────────────────────────
            <div class="flex border-b border-border mb-4">
                <button class=move || tab_class("overview") on:click=move |_| set_active_tab.set("overview")>"Overview"</button>
                <button class=move || tab_class("storage")  on:click=move |_| set_active_tab.set("storage") >"Storage"</button>
                <button class=move || tab_class("network")  on:click=move |_| set_active_tab.set("network") >"Network"</button>
                <button class=move || tab_class("gitops")   on:click=move |_| set_active_tab.set("gitops")  >"GitOps"</button>
                <button class=move || tab_class("audit")    on:click=move |_| set_active_tab.set("audit")   >"AI Audit"</button>
            </div>

            // ── Tab: Overview ─────────────────────────────────────────────────
            {move || (active_tab.get() == "overview").then(|| {
                if let Some(cluster) = cluster_metrics.get() {
                    let cpu_hist  = cpu_history.get().iter().copied().collect::<Vec<_>>();
                    let mem_hist  = memory_history.get().iter().copied().collect::<Vec<_>>();
                    let disk_hist = disk_history.get().iter().copied().collect::<Vec<_>>();
                    let rx_hist   = network_rx_history.get().iter().copied().collect::<Vec<_>>();
                    let tx_hist   = network_tx_history.get().iter().copied().collect::<Vec<_>>();
                    view! {
                        <div>
                            <div class="flex gap-6 mb-4 text-sm flex-wrap">
                                <div class="flex items-baseline gap-2">
                                    <span class="text-2xl font-bold text-accent">{cluster.pod_count}</span>
                                    <span class="text-charcoal-lighter">"pods"</span>
                                </div>
                                <div class="flex items-baseline gap-2">
                                    <span class="text-2xl font-bold text-green-600">
                                        {cluster.healthy_node_count} "/" {cluster.node_count}
                                    </span>
                                    <span class="text-charcoal-lighter">"nodes"</span>
                                </div>
                            </div>
                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 mb-4">
                                <LineChart data=cpu_hist  title="CPU Usage".to_string()     color="#ef4444".to_string() />
                                <LineChart data=mem_hist  title="Memory Usage".to_string()  color="#3b82f6".to_string() />
                                <LineChart data=disk_hist title="Storage Usage".to_string() color="#8b5cf6".to_string() />
                            </div>
                            <StackedAreaChart data_rx=rx_hist data_tx=tx_hist title="Network".to_string() />
                            {(!node_metrics.get().is_empty()).then(|| view! {
                                <div class="mt-4">
                                    <h3 class="text-sm font-medium text-charcoal-lighter mb-2">"Nodes"</h3>
                                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                                        <For
                                            each=move || node_metrics.get()
                                            key=|node| node.name.clone()
                                            children=move |node| view! { <NodeCard node=node /> }
                                        />
                                    </div>
                                </div>
                            })}
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <p class="text-center text-charcoal-light">"Connecting to metrics stream..."</p>
                    }.into_any()
                }
            })}

            // ── Tab: Storage ──────────────────────────────────────────────────
            {move || (active_tab.get() == "storage").then(|| {
                let db_info = cluster_metrics.get().and_then(|c| c.db_info);
                view! {
                    <div>
                        {db_info.map(|info| {
                            let total: i64 = info.databases.iter().map(|d| d.size_bytes).sum();
                            view! {
                                <div class="bg-surface rounded-lg shadow-sm p-4 border border-border mb-4">
                                    <h3 class="text-xs font-medium text-charcoal-lighter mb-3">"Postgres"</h3>
                                    <div class="flex items-baseline gap-2 mb-2">
                                        <span class="text-2xl font-bold text-blue-500">{fmt_db_size(total)}</span>
                                        {info.pvc_capacity_bytes.map(|cap| view! {
                                            <span class="text-charcoal-lighter">"/ " {fmt_db_size(cap)} " PVC"</span>
                                        })}
                                    </div>
                                    <div class="space-y-1">
                                        {info.databases.into_iter().map(|db| {
                                            let pct = info.pvc_capacity_bytes
                                                .filter(|&cap| cap > 0)
                                                .map(|cap| (db.size_bytes as f64 / cap as f64 * 100.0).min(100.0))
                                                .unwrap_or(0.0);
                                            view! {
                                                <div class="flex items-center gap-3 text-xs">
                                                    <span class="text-charcoal w-32 truncate font-mono">{db.name}</span>
                                                    <div class="flex-1 bg-background rounded-full h-1.5">
                                                        <div
                                                            class="bg-blue-500 h-1.5 rounded-full"
                                                            style=format!("width: {pct:.1}%")
                                                        />
                                                    </div>
                                                    <span class="text-charcoal-lighter w-12 text-right">{fmt_db_size(db.size_bytes)}</span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            }
                        })}
                        {if let Some(ceph) = ceph_status.get() {
                            view! { <CephStatusPanel ceph=ceph /> }.into_any()
                        } else {
                            view! {
                                <p class="text-center text-charcoal-light py-8">"No Ceph data yet..."</p>
                            }.into_any()
                        }}
                    </div>
                }.into_any()
            })}

            // ── Tab: Network ──────────────────────────────────────────────────
            {move || (active_tab.get() == "network").then(|| {
                let insights = network_insights.get();
                if insights.is_empty() {
                    view! {
                        <p class="text-center text-charcoal-light py-8">
                            "No spike events recorded yet."
                        </p>
                    }.into_any()
                } else {
                    view! { <NetworkInsightsPanel insights=insights /> }.into_any()
                }
            })}

            // ── Tab: GitOps ───────────────────────────────────────────────────
            {move || (active_tab.get() == "gitops").then(|| {
                let resources = gitops.get();
                if resources.is_empty() {
                    view! {
                        <p class="text-center text-charcoal-light py-8">"No Flux resources found."</p>
                    }.into_any()
                } else {
                    view! { <GitOpsPanel resources=resources /> }.into_any()
                }
            })}

            // ── Tab: AI Audit ─────────────────────────────────────────────────
            {move || (active_tab.get() == "audit").then(|| {
                let entries = audit_log.get();
                if entries.is_empty() {
                    view! {
                        <p class="text-center text-charcoal-light py-8">
                            "No Claude API calls recorded yet."
                        </p>
                    }.into_any()
                } else {
                    view! { <ClaudeAuditPanel entries=entries /> }.into_any()
                }
            })}

        </div>
    }
}

#[component]
fn LineChart(
    data: Vec<f64>,
    title: String,
    color: String,
    #[prop(default = "%".to_string())] unit: String,
) -> impl IntoView {
    let max_val = data.iter().cloned().fold(0.0f64, f64::max).max(1.0);
    let min_val = data.iter().cloned().fold(100.0f64, f64::min).min(0.0);
    let range = (max_val - min_val).max(1.0);

    let points: Vec<(f64, f64)> = data
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = (i as f64 / (data.len() - 1).max(1) as f64) * 100.0;
            let y = 30.0 - ((val - min_val) / range * 30.0);
            (x, y)
        })
        .collect();

    let path_data = if !points.is_empty() {
        let mut d = format!("M {} {}", points[0].0, points[0].1);
        for (x, y) in points.iter().skip(1) {
            d.push_str(&format!(" L {} {}", x, y));
        }
        d
    } else {
        String::new()
    };

    let current_val = data.last().copied().unwrap_or(0.0);

    view! {
        <div class="bg-surface p-3 rounded-lg shadow-sm border border-border">
            <div class="flex justify-between items-center mb-1.5">
                <h3 class="text-xs font-medium text-charcoal-lighter">{title}</h3>
                <span class="text-base font-bold" style=format!("color: {}", color)>
                    {format!("{:.1}{}", current_val, unit)}
                </span>
            </div>
            <svg viewBox="0 0 100 30" class="w-full h-12" preserveAspectRatio="none">
                <path
                    d={path_data}
                    fill="none"
                    stroke={color.clone()}
                    stroke-width="0.5"
                    stroke-linejoin="round"
                    stroke-linecap="round"
                    vector-effect="non-scaling-stroke"
                />
            </svg>
        </div>
    }
}

#[component]
fn StackedAreaChart(
    data_rx: Vec<f64>,
    data_tx: Vec<f64>,
    title: String,
) -> impl IntoView {
    let max_val = data_rx.iter().chain(data_tx.iter())
        .cloned()
        .fold(0.0f64, f64::max)
        .max(1.0);

    let points_rx: Vec<(f64, f64)> = data_rx
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = (i as f64 / (data_rx.len() - 1).max(1) as f64) * 100.0;
            let y = 30.0 - (val / max_val * 30.0);
            (x, y)
        })
        .collect();

    let points_tx: Vec<(f64, f64)> = data_tx
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = (i as f64 / (data_tx.len() - 1).max(1) as f64) * 100.0;
            let y = 30.0 - (val / max_val * 30.0);
            (x, y)
        })
        .collect();

    let path_rx = if !points_rx.is_empty() {
        let mut d = format!("M {} {}", points_rx[0].0, points_rx[0].1);
        for (x, y) in points_rx.iter().skip(1) {
            d.push_str(&format!(" L {} {}", x, y));
        }
        d.push_str(" L 100 30 L 0 30 Z");
        d
    } else {
        String::new()
    };

    let path_tx = if !points_tx.is_empty() {
        let mut d = format!("M {} {}", points_tx[0].0, points_tx[0].1);
        for (x, y) in points_tx.iter().skip(1) {
            d.push_str(&format!(" L {} {}", x, y));
        }
        d.push_str(" L 100 30 L 0 30 Z");
        d
    } else {
        String::new()
    };

    let current_rx = data_rx.last().copied().unwrap_or(0.0);
    let current_tx = data_tx.last().copied().unwrap_or(0.0);

    view! {
        <div class="bg-surface p-3 rounded-lg shadow-sm border border-border">
            <div class="flex justify-between items-center mb-1.5">
                <h3 class="text-xs font-medium text-charcoal-lighter">{title}</h3>
                <div class="flex gap-3 text-xs font-semibold">
                    <span class="text-accent">"↓ " {format!("{:.1}", current_rx)}</span>
                    <span class="text-amber-500">"↑ " {format!("{:.1}", current_tx)}</span>
                    <span class="text-charcoal-lighter">"Mbps"</span>
                </div>
            </div>
            <svg viewBox="0 0 100 30" class="w-full h-12" preserveAspectRatio="none">
                <path
                    d={path_tx}
                    fill="#f59e0b"
                    fill-opacity="0.4"
                />
                <path
                    d={path_rx}
                    fill="#3b82f6"
                    fill-opacity="0.6"
                />
            </svg>
        </div>
    }
}
