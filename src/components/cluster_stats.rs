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

#[server(GetClusterMetrics, "/api")]
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

    // Disk metrics (convert bytes to GB)
    let disk_used = parse_prometheus_value(
        "sum(kubelet_volume_stats_used_bytes) / 1024 / 1024 / 1024"
    ).await?;
    let disk_total = parse_prometheus_value(
        "sum(kubelet_volume_stats_capacity_bytes) / 1024 / 1024 / 1024"
    ).await?;

    // Pod count
    let pod_count = parse_prometheus_value(
        "count(kube_pod_info{pod!=\"\"})"
    ).await? as u32;

    // Node count
    let node_count = parse_prometheus_value(
        "count(kube_node_info)"
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
        network_rx_mbps: network_rx,
        network_tx_mbps: network_tx,
    })
}

#[server(GetNodeMetrics, "/api")]
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

#[component]
fn MetricCard(
    title: &'static str,
    value: String,
    subtitle: String,
    #[prop(default = None)] percentage: Option<f64>,
) -> impl IntoView {
    view! {
        <div class="bg-white rounded-lg shadow-md p-6 border border-gray-200">
            <h3 class="text-sm font-medium text-gray-500 uppercase tracking-wide">{title}</h3>
            <div class="mt-2">
                <p class="text-3xl font-bold text-charcoal">{value}</p>
                <p class="text-sm text-gray-600 mt-1">{subtitle}</p>
            </div>
            {percentage.map(|pct| {
                let bar_width = format!("{}%", pct);
                let color_class = if pct > 90.0 {
                    "bg-red-500"
                } else if pct > 75.0 {
                    "bg-yellow-500"
                } else {
                    "bg-green-500"
                };
                view! {
                    <div class="mt-3">
                        <div class="w-full bg-gray-200 rounded-full h-2">
                            <div class={format!("h-2 rounded-full {}", color_class)} style={format!("width: {}", bar_width)}></div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn NodeCard(node: NodeMetric) -> impl IntoView {
    let mem_pct = (node.memory_usage_gb / node.memory_total_gb * 100.0).min(100.0);

    view! {
        <div class="bg-white rounded-lg shadow-sm p-4 border border-gray-200">
            <h4 class="font-medium text-charcoal mb-2">{node.name}</h4>
            <div class="space-y-2 text-sm">
                <div>
                    <div class="flex justify-between mb-1">
                        <span class="text-gray-600">"CPU"</span>
                        <span class="font-medium">{format!("{:.1}%", node.cpu_usage_percent)}</span>
                    </div>
                    <div class="w-full bg-gray-200 rounded-full h-1.5">
                        <div
                            class="bg-blue-500 h-1.5 rounded-full"
                            style={format!("width: {}%", node.cpu_usage_percent)}
                        ></div>
                    </div>
                </div>
                <div>
                    <div class="flex justify-between mb-1">
                        <span class="text-gray-600">"Memory"</span>
                        <span class="font-medium">{format!("{:.1} / {:.1} GB", node.memory_usage_gb, node.memory_total_gb)}</span>
                    </div>
                    <div class="w-full bg-gray-200 rounded-full h-1.5">
                        <div
                            class="bg-purple-500 h-1.5 rounded-full"
                            style={format!("width: {}%", mem_pct)}
                        ></div>
                    </div>
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
    let (cluster_metrics, set_cluster_metrics) = signal(None::<ClusterMetrics>);
    let (node_metrics, set_node_metrics) = signal(Vec::<NodeMetric>::new());
    // Note: set_error is used in closures below, but Rust can't see through
    // the .forget() pattern required for WASM event handlers
    #[allow(unused_variables)]
    let (error, set_error) = signal(None::<String>);

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
                            if let Some(cluster) = update.cluster {
                                set_cluster_metrics.set(Some(cluster));
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
        <div class="w-full bg-gradient-to-br from-gray-50 to-gray-100 py-8 px-4 rounded-xl mb-8">
            <h2 class="text-2xl font-bold text-charcoal mb-6 text-center">
                "Homelab Cluster Metrics"
                <span class="text-xs ml-2 text-green-600">"● Live"</span>
            </h2>

            {move || {
                if let Some(err) = error.get() {
                    view! {
                        <div class="text-center text-red-500 p-4 mb-4">
                            <p>"Connection error: " {err}</p>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            {move || {
                if let Some(m) = cluster_metrics.get() {
                    let cpu_pct = m.cpu_usage_percent;
                    let mem_pct = (m.memory_usage_gb / m.memory_total_gb * 100.0).min(100.0);
                    let disk_pct = (m.disk_usage_gb / m.disk_total_gb * 100.0).min(100.0);

                    view! {
                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
                            <MetricCard
                                title="CPU"
                                value=format!("{:.1}%", cpu_pct)
                                subtitle=format!("{:.1} cores total", m.cpu_total_cores)
                                percentage=Some(cpu_pct)
                            />
                            <MetricCard
                                title="Memory"
                                value=format!("{:.1} GB", m.memory_usage_gb)
                                subtitle=format!("{:.1} GB total", m.memory_total_gb)
                                percentage=Some(mem_pct)
                            />
                            <MetricCard
                                title="Storage"
                                value=format!("{:.1} GB", m.disk_usage_gb)
                                subtitle=format!("{:.1} GB total", m.disk_total_gb)
                                percentage=Some(disk_pct)
                            />
                            <MetricCard
                                title="Pods"
                                value=format!("{}", m.pod_count)
                                subtitle=format!("{} nodes", m.node_count)
                            />
                        </div>
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
                            <MetricCard
                                title="Network In"
                                value=format!("{:.2} Mbps", m.network_rx_mbps)
                                subtitle="Average over 5m".to_string()
                            />
                            <MetricCard
                                title="Network Out"
                                value=format!("{:.2} Mbps", m.network_tx_mbps)
                                subtitle="Average over 5m".to_string()
                            />
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <p class="text-center text-gray-500">"Connecting to metrics stream..."</p>
                    }.into_any()
                }
            }}

            {move || {
                let nodes = node_metrics.get();
                if !nodes.is_empty() {
                    view! {
                        <div>
                            <h3 class="text-xl font-semibold text-charcoal mb-4">"Node Metrics"</h3>
                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                                <For
                                    each=move || node_metrics.get()
                                    key=|node| node.name.clone()
                                    children=move |node| {
                                        view! { <NodeCard node=node /> }
                                    }
                                />
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}
        </div>
    }
}
