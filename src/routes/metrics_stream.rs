use actix_web::web::Data;
use actix_web_lab::sse::{self, Sse};
use serde::Serialize;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::components::cluster_stats::{DbEntry, DbInfo};
use crate::network_spike::NetworkSpikeDetector;
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
    pub db_info: Option<DbInfo>,
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
pub struct MetricsUpdate {
    pub cluster: Option<ClusterMetrics>,
    pub nodes: Vec<NodeMetric>,
    pub ceph: Option<CephStatus>,
    pub latest_insight: Option<NetworkInsight>,
}

async fn parse_prometheus_value(query: &str) -> Result<f64, anyhow::Error> {
    let data = query_prometheus(query).await?;

    if let Some(metric) = data.data.result.first() {
        metric.value.1.parse::<f64>().map_err(|e| anyhow::anyhow!("Failed to parse value: {}", e))
    } else {
        Ok(0.0)
    }
}

async fn fetch_cluster_metrics() -> Result<ClusterMetrics, anyhow::Error> {
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

async fn fetch_node_metrics() -> Result<Vec<NodeMetric>, anyhow::Error> {
    // Get all nodes with their metrics
    let cpu_data = query_prometheus(
        "sum by (node) (rate(container_cpu_usage_seconds_total{container!=\"\"}[5m]))"
    ).await?;

    let memory_data = query_prometheus(
        "sum by (node) (container_memory_working_set_bytes{container!=\"\"}) / 1024 / 1024 / 1024"
    ).await?;

    let memory_total_data = query_prometheus(
        "sum by (node) (machine_memory_bytes) / 1024 / 1024 / 1024"
    ).await?;

    let cpu_capacity_data = query_prometheus(
        "sum by (node) (machine_cpu_cores)"
    ).await?;

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

pub async fn metrics_stream(
    pool: Option<Data<PgPool>>,
    spike_detector: Data<Mutex<NetworkSpikeDetector>>,
) -> impl actix_web::Responder {
    let stream = IntervalStream::new(interval(Duration::from_secs(1)))
        .then(move |_| {
            let pool = pool.clone();
            let spike_detector = spike_detector.clone();
            async move {
            // Fetch cluster, node, and ceph metrics
            let cluster_metrics = fetch_cluster_metrics().await.ok();
            let node_metrics = fetch_node_metrics().await.unwrap_or_default();
            let ceph = fetch_ceph_status().await;

            // Check for a network spike and explain it asynchronously.
            if let Some(ref c) = cluster_metrics {
                let spike = spike_detector.lock().await.check(c.network_tx_mbps);
                if let Some((spike_mbps, baseline_mbps)) = spike {
                    let pool_opt = pool.clone();
                    let detector_ref = spike_detector.clone();
                    tokio::spawn(async move {
                        match crate::network_spike::explain_spike(spike_mbps, baseline_mbps).await {
                            Ok((pods, explanation, significance)) => {
                                tracing::info!(
                                    "Spike significance={significance}/10, \
                                     multiplier/floor will be adjusted accordingly"
                                );

                                let top_pods_json = serde_json::to_value(
                                    pods.iter()
                                        .map(|p| serde_json::json!({
                                            "namespace": p.namespace,
                                            "pod": p.pod,
                                            "mbps": p.mbps,
                                        }))
                                        .collect::<Vec<_>>()
                                ).unwrap_or_default();

                                // Apply feedback to in-memory thresholds.
                                let (new_mult, new_floor) = {
                                    let mut det = detector_ref.lock().await;
                                    det.apply_feedback(significance);
                                    (det.multiplier, det.floor_mbps)
                                };
                                tracing::info!(
                                    "Thresholds after feedback: multiplier={new_mult:.2} floor={new_floor:.1} Mbps"
                                );

                                // Persist to DB so all pods and restarts pick up the new values.
                                if let Some(ref pool) = pool_opt {
                                    let _ = crate::db::save_spike_config(pool, new_mult, new_floor).await;
                                    let _ = crate::db::insert_network_insight(
                                        pool,
                                        spike_mbps,
                                        baseline_mbps,
                                        &top_pods_json,
                                        &explanation,
                                    ).await;
                                }
                            }
                            Err(e) => tracing::warn!("Failed to explain network spike: {}", e),
                        }
                    });
                }
            }

            // Include the most recent stored insight (if any) so the client
            // picks it up without needing a separate poll.
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
            };

            // Serialize to JSON and create SSE event
            match serde_json::to_string(&update) {
                Ok(json) => Ok::<_, anyhow::Error>(sse::Event::Data(sse::Data::new(json))),
                Err(e) => {
                    tracing::error!("Failed to serialize metrics: {}", e);
                    Ok(sse::Event::Data(sse::Data::new("{}")))
                }
            }
        }})
        .filter_map(|result| match result {
            Ok(event) => Some(Ok::<_, actix_web::Error>(event)),
            Err(e) => {
                tracing::error!("Error in metrics stream: {}", e);
                None
            }
        });

    Sse::from_stream(stream)
}
