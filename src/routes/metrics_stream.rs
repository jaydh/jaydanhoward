use axum::{
    extract::Extension,
    response::sse::{Event, KeepAlive, Sse},
};
use serde::Serialize;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::components::cluster_stats::{PvcEntry, StorageInfo};
use crate::prometheus_client::query_prometheus;

#[derive(Clone, Debug, Serialize)]
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
    pub storage: Option<StorageInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NodeMetric {
    pub name: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_gb: f64,
    pub memory_total_gb: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CephStatus {
    pub health: u32,
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
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub read_iops: f64,
    pub write_iops: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PodTraffic {
    pub namespace: String,
    pub pod: String,
    pub mbps: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkInsight {
    pub id: i64,
    pub occurred_at: String,
    pub spike_tx_mbps: f64,
    pub baseline_tx_mbps: f64,
    pub top_pods: Vec<PodTraffic>,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SdSyncStatus {
    pub active: bool,
    pub last_sync_end_timestamp: f64,
    pub last_sync_duration_seconds: f64,
    pub last_sync_files_copied: u64,
    pub last_sync_files_skipped: u64,
    pub last_sync_bytes_copied: u64,
    pub syncs_completed_total: u64,
    pub syncs_errored_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_files_copied: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_files_skipped: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_bytes_copied: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetricsUpdate {
    pub cluster: Option<ClusterMetrics>,
    pub nodes: Vec<NodeMetric>,
    pub ceph: Option<CephStatus>,
    pub latest_insight: Option<NetworkInsight>,
    pub sd_sync: Option<SdSyncStatus>,
}

async fn parse_prometheus_value(query: &str) -> Result<f64, anyhow::Error> {
    let data = query_prometheus(query).await?;
    if let Some(metric) = data.data.result.first() {
        metric.value.1.parse::<f64>().map_err(|e| anyhow::anyhow!("Failed to parse value: {e}"))
    } else {
        Ok(0.0)
    }
}

async fn fetch_cluster_metrics() -> Result<ClusterMetrics, anyhow::Error> {
    let cpu_used = parse_prometheus_value(
        "sum(rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))"
    ).await?;
    let cpu_total = parse_prometheus_value("sum(machine_cpu_cores)").await?;

    let memory_used = parse_prometheus_value(
        "sum(container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024"
    ).await?;
    let memory_total = parse_prometheus_value(
        "sum(machine_memory_bytes) / 1024 / 1024 / 1024"
    ).await?;

    let disk_used = parse_prometheus_value(
        "ceph_cluster_total_used_bytes / 1024 / 1024 / 1024"
    ).await?;
    let disk_total = parse_prometheus_value(
        "ceph_cluster_total_bytes / 1024 / 1024 / 1024"
    ).await?;

    let pod_count = parse_prometheus_value(
        "count(kube_pod_info{pod!=\"\"})"
    ).await? as u32;

    let node_count = parse_prometheus_value("count(kube_node_info)").await? as u32;

    let healthy_node_count = parse_prometheus_value(
        "sum(kube_node_status_condition{condition=\"Ready\",status=\"true\"})"
    ).await? as u32;

    let network_rx = parse_prometheus_value(
        "sum(rate(node_network_receive_bytes_total{\
          device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"\
        }[5m])) * 8 / 1000000"
    ).await?;
    let network_tx = parse_prometheus_value(
        "sum(rate(node_network_transmit_bytes_total{\
          device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"\
        }[5m])) * 8 / 1000000"
    ).await?;

    let storage = {
        use std::collections::HashMap;

        let empty = || crate::prometheus_client::PrometheusData {
            status: String::new(),
            data: crate::prometheus_client::PrometheusResult {
                result_type: String::new(),
                result: vec![],
            },
        };

        let (cap_data, used_data) = tokio::join!(
            query_prometheus(
                "max by (namespace, persistentvolumeclaim) (kubelet_volume_stats_capacity_bytes)"
            ),
            query_prometheus(
                "max by (namespace, persistentvolumeclaim) (kubelet_volume_stats_used_bytes)"
            ),
        );

        let cap_map: HashMap<(String, String), i64> = cap_data
            .unwrap_or_else(|_| empty())
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

        let mut pvcs: Vec<PvcEntry> = used_data
            .unwrap_or_else(|_| empty())
            .data
            .result
            .into_iter()
            .filter_map(|m| {
                let ns = m.metric.get("namespace")?.clone();
                let pvc = m.metric.get("persistentvolumeclaim")?.clone();
                let used = m.value.1.parse::<f64>().ok()? as i64;
                let capacity = *cap_map.get(&(ns.clone(), pvc.clone())).unwrap_or(&0);
                Some(PvcEntry {
                    namespace: ns,
                    name: pvc,
                    used_bytes: used,
                    capacity_bytes: capacity,
                })
            })
            .collect();

        pvcs.sort_by(|a, b| b.used_bytes.cmp(&a.used_bytes));

        if pvcs.is_empty() {
            None
        } else {
            Some(StorageInfo { pvcs })
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
        storage,
    })
}

async fn fetch_node_metrics() -> Result<Vec<NodeMetric>, anyhow::Error> {
    let cpu_data = query_prometheus(
        "sum by (node) (rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))"
    ).await?;

    let memory_data = query_prometheus(
        "sum by (node) (container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024"
    ).await?;

    let memory_total_data = query_prometheus(
        "sum by (node) (machine_memory_bytes) / 1024 / 1024 / 1024"
    ).await?;

    let cpu_capacity_data = query_prometheus("sum by (node) (machine_cpu_cores)").await?;

    let mut nodes = std::collections::HashMap::new();

    for metric in cpu_data.data.result {
        if let Some(node) = metric.metric.get("node") {
            let cpu_used: f64 = metric.value.1.parse().unwrap_or(0.0);
            nodes
                .entry(node.clone())
                .or_insert(NodeMetric {
                    name: node.clone(),
                    cpu_usage_percent: 0.0,
                    memory_usage_gb: 0.0,
                    memory_total_gb: 0.0,
                })
                .cpu_usage_percent = cpu_used;
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

async fn fetch_ceph_status() -> CephStatus {
    let health = parse_prometheus_value("ceph_health_status").await.unwrap_or(0.0) as u32;
    let mon_quorum = parse_prometheus_value("sum(ceph_mon_quorum_status == 1)").await.unwrap_or(0.0) as u32;
    let mon_total = parse_prometheus_value("count(ceph_mon_quorum_status)").await.unwrap_or(0.0) as u32;
    let mgr_active = parse_prometheus_value("sum(ceph_mgr_status == 1)").await.unwrap_or(0.0) as u32;
    let mgr_standby = parse_prometheus_value("sum(ceph_mgr_status == 0)").await.unwrap_or(0.0) as u32;
    let mds_up = parse_prometheus_value("count(ceph_mds_metadata{fs_state=\"up:active\"})").await.unwrap_or(0.0) as u32;
    let mds_standby = parse_prometheus_value("count(ceph_mds_metadata{fs_state=~\"up:standby.*\"})").await.unwrap_or(0.0) as u32;
    let osd_up = parse_prometheus_value("count(ceph_osd_up == 1)").await.unwrap_or(0.0) as u32;
    let osd_in = parse_prometheus_value("count(ceph_osd_in == 1)").await.unwrap_or(0.0) as u32;
    let osd_total = parse_prometheus_value("count(ceph_osd_up)").await.unwrap_or(0.0) as u32;
    let rgw_count = parse_prometheus_value("count(ceph_rgw_metadata)").await.unwrap_or(0.0) as u32;
    let volumes_total = parse_prometheus_value("count(ceph_fs_metadata)").await.unwrap_or(0.0) as u32;
    let volumes_healthy = volumes_total;
    let pool_count = parse_prometheus_value("ceph_osdmap_num_pools").await.unwrap_or(0.0) as u32;
    let pg_total = parse_prometheus_value("ceph_osdmap_num_pg").await.unwrap_or(0.0) as u32;
    let pg_clean = parse_prometheus_value("ceph_pg_state{state=\"active+clean\"}").await.unwrap_or(0.0) as u32;
    let pg_degraded = parse_prometheus_value("sum(ceph_pg_state{state=~\".*degraded.*\"})").await.unwrap_or(0.0) as u32;
    let pg_recovering = parse_prometheus_value("sum(ceph_pg_state{state=~\".*recovering.*\"})").await.unwrap_or(0.0) as u32;
    let pg_remapped = parse_prometheus_value("sum(ceph_pg_state{state=~\".*remapped.*\"})").await.unwrap_or(0.0) as u32;
    let pg_scrubbing = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing(?!\\\\+deep).*\"})").await.unwrap_or(0.0) as u32;
    let pg_deep_scrub = parse_prometheus_value("sum(ceph_pg_state{state=~\".*scrubbing\\\\+deep.*\"})").await.unwrap_or(0.0) as u32;
    let objects_count = parse_prometheus_value("sum(ceph_pool_objects_total)").await.unwrap_or(0.0);
    let data_used_bytes = parse_prometheus_value("ceph_cluster_total_used_bytes").await.unwrap_or(0.0);
    let data_total_bytes = parse_prometheus_value("ceph_cluster_total_bytes").await.unwrap_or(0.0);
    let data_avail_bytes = (data_total_bytes - data_used_bytes).max(0.0);
    let read_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_r_out_bytes[5m]))").await.unwrap_or(0.0);
    let write_bytes_per_sec = parse_prometheus_value("sum(irate(ceph_osd_op_w_in_bytes[5m]))").await.unwrap_or(0.0);
    let read_iops = parse_prometheus_value("sum(irate(ceph_osd_op_r[5m]))").await.unwrap_or(0.0);
    let write_iops = parse_prometheus_value("sum(irate(ceph_osd_op_w[5m]))").await.unwrap_or(0.0);

    CephStatus {
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
    }
}

const SD_SYNC_STATUS_URL: &str = "http://sd-sync.media.svc.cluster.local:9105/status";

#[derive(serde::Deserialize)]
struct SdSyncStatusResponse {
    current_file: Option<String>,
    current_progress: Option<SdSyncProgress>,
}

#[derive(serde::Deserialize)]
struct SdSyncProgress {
    files_copied: u64,
    files_skipped: u64,
    bytes_copied: u64,
}

async fn fetch_sd_sync_live() -> Option<SdSyncStatusResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    client.get(SD_SYNC_STATUS_URL).send().await.ok()?.json().await.ok()
}

async fn fetch_sd_sync_status() -> Option<SdSyncStatus> {
    let (prom, live) = tokio::join!(
        async {
            Some((
                parse_prometheus_value("sd_sync_active").await.ok()? != 0.0,
                parse_prometheus_value("sd_sync_last_sync_end_timestamp_seconds").await.unwrap_or(0.0),
                parse_prometheus_value("sd_sync_last_sync_duration_seconds").await.unwrap_or(0.0),
                parse_prometheus_value("sd_sync_last_sync_files_copied").await.unwrap_or(0.0) as u64,
                parse_prometheus_value("sd_sync_last_sync_files_skipped").await.unwrap_or(0.0) as u64,
                parse_prometheus_value("sd_sync_last_sync_bytes_copied").await.unwrap_or(0.0) as u64,
                parse_prometheus_value("sd_sync_syncs_completed_total").await.unwrap_or(0.0) as u64,
                parse_prometheus_value("sd_sync_syncs_errored_total").await.unwrap_or(0.0) as u64,
            ))
        },
        fetch_sd_sync_live(),
    );

    let (
        active,
        last_sync_end_timestamp,
        last_sync_duration_seconds,
        last_sync_files_copied,
        last_sync_files_skipped,
        last_sync_bytes_copied,
        syncs_completed_total,
        syncs_errored_total,
    ) = prom?;

    let (current_file, current_files_copied, current_files_skipped, current_bytes_copied) =
        match live {
            Some(s) => (
                s.current_file,
                s.current_progress.as_ref().map(|p| p.files_copied),
                s.current_progress.as_ref().map(|p| p.files_skipped),
                s.current_progress.as_ref().map(|p| p.bytes_copied),
            ),
            None => (None, None, None, None),
        };

    Some(SdSyncStatus {
        active,
        last_sync_end_timestamp,
        last_sync_duration_seconds,
        last_sync_files_copied,
        last_sync_files_skipped,
        last_sync_bytes_copied,
        syncs_completed_total,
        syncs_errored_total,
        current_file,
        current_files_copied,
        current_files_skipped,
        current_bytes_copied,
    })
}

pub async fn metrics_stream(
    Extension(pool): Extension<Option<Arc<PgPool>>>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(interval(Duration::from_secs(1))).then(move |_| {
        let pool = pool.clone();
        async move {
            let (cluster_metrics, node_metrics, ceph, sd_sync) = tokio::join!(
                fetch_cluster_metrics(),
                fetch_node_metrics(),
                fetch_ceph_status(),
                fetch_sd_sync_status(),
            );
            let cluster_metrics = cluster_metrics.ok();
            let node_metrics = node_metrics.unwrap_or_default();

            let latest_insight = if let Some(ref p) = pool {
                crate::db::get_recent_network_insights(p, 1)
                    .await
                    .ok()
                    .and_then(|mut rows| rows.pop())
                    .map(|r| NetworkInsight {
                        id: r.id,
                        occurred_at: r.occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                        spike_tx_mbps: r.spike_tx_mbps,
                        baseline_tx_mbps: r.baseline_tx_mbps,
                        top_pods: serde_json::from_value::<Vec<serde_json::Value>>(r.top_pods)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|v| PodTraffic {
                                namespace: v["namespace"].as_str().unwrap_or("").to_string(),
                                pod: v["pod"].as_str().unwrap_or("").to_string(),
                                mbps: v["mbps"].as_f64().unwrap_or(0.0),
                            })
                            .collect(),
                        explanation: r.explanation,
                    })
            } else {
                None
            };

            let update = MetricsUpdate {
                cluster: cluster_metrics,
                nodes: node_metrics,
                ceph: Some(ceph),
                latest_insight,
                sd_sync,
            };

            let json = serde_json::to_string(&update).unwrap_or_else(|_| "{}".to_string());
            Ok::<Event, Infallible>(Event::default().data(json))
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
