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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsUpdate {
    pub cluster: Option<ClusterMetrics>,
    pub nodes: Vec<NodeMetric>,
}

#[component]
pub fn ClusterStats() -> impl IntoView {
    use std::collections::VecDeque;

    let (cluster_metrics, set_cluster_metrics) = signal(None::<ClusterMetrics>);
    let (node_metrics, set_node_metrics) = signal(Vec::<NodeMetric>::new());

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
        });
    });

    view! {
        <div class="w-full bg-gray py-6 px-4 rounded-xl mb-8">
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-bold text-charcoal">
                    "Homelab Cluster"
                </h2>
                <div class="text-right">
                    <span class="text-xs text-green-600">"● Live"</span>
                    {move || {
                        last_refresh.get().map(|time| view! {
                            <span class="text-xs text-charcoal-lighter ml-2">{time}</span>
                        })
                    }}
                </div>
            </div>

            {move || {
                error.get().map(|err| view! {
                    <div class="text-center text-red-500 p-4 mb-4">
                        <p>"Connection error: " {err}</p>
                    </div>
                })
            }}

            {move || {
                if let Some(cluster) = cluster_metrics.get() {
                    let cpu_hist = cpu_history.get().iter().copied().collect::<Vec<_>>();
                    let mem_hist = memory_history.get().iter().copied().collect::<Vec<_>>();
                    let disk_hist = disk_history.get().iter().copied().collect::<Vec<_>>();
                    let rx_hist = network_rx_history.get().iter().copied().collect::<Vec<_>>();
                    let tx_hist = network_tx_history.get().iter().copied().collect::<Vec<_>>();

                    view! {
                        <div class="flex gap-6 mb-4 text-sm">
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
                            <LineChart
                                data=cpu_hist
                                title="CPU Usage".to_string()
                                color="#ef4444".to_string()
                            />
                            <LineChart
                                data=mem_hist
                                title="Memory Usage".to_string()
                                color="#3b82f6".to_string()
                            />
                            <LineChart
                                data=disk_hist
                                title="Storage Usage".to_string()
                                color="#8b5cf6".to_string()
                            />
                        </div>
                        <StackedAreaChart
                            data_rx=rx_hist
                            data_tx=tx_hist
                            title="Network".to_string()
                        />
                    }.into_any()
                } else {
                    view! {
                        <p class="text-center text-charcoal-light">"Connecting to metrics stream..."</p>
                    }.into_any()
                }
            }}

            {move || {
                (!node_metrics.get().is_empty()).then(|| view! {
                    <div class="mt-4">
                        <h3 class="text-sm font-medium text-charcoal-lighter mb-2">"Nodes"</h3>
                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                            <For
                                each=move || node_metrics.get()
                                key=|node| node.name.clone()
                                children=move |node| {
                                    view! { <NodeCard node=node /> }
                                }
                            />
                        </div>
                    </div>
                })
            }}
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
            let y = 100.0 - ((val - min_val) / range * 100.0);
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
            let y = 100.0 - (val / max_val * 100.0);
            (x, y)
        })
        .collect();

    let points_tx: Vec<(f64, f64)> = data_tx
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = (i as f64 / (data_tx.len() - 1).max(1) as f64) * 100.0;
            let y = 100.0 - (val / max_val * 100.0);
            (x, y)
        })
        .collect();

    let path_rx = if !points_rx.is_empty() {
        let mut d = format!("M {} {}", points_rx[0].0, points_rx[0].1);
        for (x, y) in points_rx.iter().skip(1) {
            d.push_str(&format!(" L {} {}", x, y));
        }
        d.push_str(" L 100 100 L 0 100 Z");
        d
    } else {
        String::new()
    };

    let path_tx = if !points_tx.is_empty() {
        let mut d = format!("M {} {}", points_tx[0].0, points_tx[0].1);
        for (x, y) in points_tx.iter().skip(1) {
            d.push_str(&format!(" L {} {}", x, y));
        }
        d.push_str(" L 100 100 L 0 100 Z");
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
