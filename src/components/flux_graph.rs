use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Shared types (compiled for both SSR and WASM) ────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReadyStatus {
    Ready,
    Failed,
    Suspended,
    Unknown,
}

impl ReadyStatus {
    fn dot_color(&self) -> &'static str {
        match self {
            Self::Ready => "#22c55e",
            Self::Failed => "#ef4444",
            Self::Suspended => "#f59e0b",
            Self::Unknown => "#6b7280",
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Suspended => "suspended",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelmReleaseInfo {
    pub name: String,
    pub chart: Option<String>,
    pub status: ReadyStatus,
    pub revision: Option<String>,
    pub message: Option<String>,
}

// Shared for both Deployments and StatefulSets
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkloadInfo {
    pub name: String,
    pub ready: u32,
    pub desired: u32,
}

impl WorkloadInfo {
    fn status(&self) -> ReadyStatus {
        if self.desired == 0 || self.ready == self.desired {
            ReadyStatus::Ready
        } else if self.ready == 0 {
            ReadyStatus::Failed
        } else {
            ReadyStatus::Unknown
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceInfo {
    pub name: String,
    pub kind: String,
    pub status: ReadyStatus,
    pub revision: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KustomizationNode {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub status: ReadyStatus,
    pub message: Option<String>,
    pub revision: Option<String>,
    pub reconciled_at: Option<String>,
    pub depends_on: Vec<String>,
    pub source: Option<SourceInfo>,
    pub helm_releases: Vec<HelmReleaseInfo>,
    pub deployments: Vec<WorkloadInfo>,
    pub stateful_sets: Vec<WorkloadInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluxGraphData {
    pub nodes: Vec<KustomizationNode>,
}

// ── Server function ───────────────────────────────────────────────────────────

#[server(name = GetFluxGraph, prefix = "/api", endpoint = "get_flux_graph")]
pub async fn get_flux_graph() -> Result<FluxGraphData, ServerFnError<String>> {
    use kube::api::{ApiResource, DynamicObject, ListParams};
    use kube::{Api, Client};

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(_) => return Ok(FluxGraphData { nodes: vec![] }),
    };

    let ks_ar = ApiResource {
        group: "kustomize.toolkit.fluxcd.io".into(),
        version: "v1".into(),
        api_version: "kustomize.toolkit.fluxcd.io/v1".into(),
        kind: "Kustomization".into(),
        plural: "kustomizations".into(),
    };
    let hr_ar = ApiResource {
        group: "helm.toolkit.fluxcd.io".into(),
        version: "v2".into(),
        api_version: "helm.toolkit.fluxcd.io/v2".into(),
        kind: "HelmRelease".into(),
        plural: "helmreleases".into(),
    };
    let deploy_ar = ApiResource {
        group: "apps".into(),
        version: "v1".into(),
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        plural: "deployments".into(),
    };
    let sts_ar = ApiResource {
        group: "apps".into(),
        version: "v1".into(),
        api_version: "apps/v1".into(),
        kind: "StatefulSet".into(),
        plural: "statefulsets".into(),
    };
    let gitrepo_ar = ApiResource {
        group: "source.toolkit.fluxcd.io".into(),
        version: "v1".into(),
        api_version: "source.toolkit.fluxcd.io/v1".into(),
        kind: "GitRepository".into(),
        plural: "gitrepositories".into(),
    };
    let ocirepo_ar = ApiResource {
        group: "source.toolkit.fluxcd.io".into(),
        version: "v1beta2".into(),
        api_version: "source.toolkit.fluxcd.io/v1beta2".into(),
        kind: "OCIRepository".into(),
        plural: "ocirepositories".into(),
    };

    let ks_api: Api<DynamicObject> = Api::all_with(client.clone(), &ks_ar);
    let hr_api: Api<DynamicObject> = Api::all_with(client.clone(), &hr_ar);
    let deploy_api: Api<DynamicObject> = Api::all_with(client.clone(), &deploy_ar);
    let sts_api: Api<DynamicObject> = Api::all_with(client.clone(), &sts_ar);
    let gitrepo_api: Api<DynamicObject> = Api::all_with(client.clone(), &gitrepo_ar);
    let ocirepo_api: Api<DynamicObject> = Api::all_with(client, &ocirepo_ar);

    let ks_list = ks_api
        .list(&ListParams::default())
        .await
        .map_err(|e| ServerFnError::ServerError(format!("list kustomizations: {e}")))?;

    let hr_list = hr_api
        .list(&ListParams::default())
        .await
        .map_err(|e| ServerFnError::ServerError(format!("list helmreleases: {e}")))?;

    let deploy_list = deploy_api
        .list(&ListParams::default())
        .await
        .map_err(|e| ServerFnError::ServerError(format!("list deployments: {e}")))?;

    let sts_list = sts_api
        .list(&ListParams::default())
        .await
        .map_err(|e| ServerFnError::ServerError(format!("list statefulsets: {e}")))?;

    let gitrepo_list = gitrepo_api
        .list(&ListParams::default())
        .await
        .map_err(|e| ServerFnError::ServerError(format!("list gitrepositories: {e}")))?;

    let ocirepo_items: Vec<DynamicObject> = ocirepo_api
        .list(&ListParams::default())
        .await
        .map(|l| l.items)
        .unwrap_or_default();

    // Build source lookup: "Kind/namespace/name" → SourceInfo
    let mut source_map: HashMap<String, SourceInfo> = HashMap::new();
    for obj in &gitrepo_list.items {
        let info = parse_source_info(obj, "GitRepository");
        let key = format!(
            "GitRepository/{}/{}",
            obj.metadata.namespace.as_deref().unwrap_or(""),
            obj.metadata.name.as_deref().unwrap_or("")
        );
        source_map.insert(key, info);
    }
    for obj in &ocirepo_items {
        let info = parse_source_info(obj, "OCIRepository");
        let key = format!(
            "OCIRepository/{}/{}",
            obj.metadata.namespace.as_deref().unwrap_or(""),
            obj.metadata.name.as_deref().unwrap_or("")
        );
        source_map.insert(key, info);
    }

    // Group HelmReleases by Kustomization.
    let mut hr_by_ks: HashMap<String, Vec<HelmReleaseInfo>> = HashMap::new();
    for obj in hr_list.items {
        let parent = flux_parent_label(&obj);
        if parent.is_empty() { continue; }
        hr_by_ks.entry(parent).or_default().push(parse_helm_release(&obj));
    }

    // Group Deployments by Kustomization.
    let mut deploy_by_ks: HashMap<String, Vec<WorkloadInfo>> = HashMap::new();
    for obj in deploy_list.items {
        let parent = flux_parent_label(&obj);
        if parent.is_empty() { continue; }
        deploy_by_ks.entry(parent).or_default().push(parse_workload(&obj));
    }

    // Group StatefulSets by Kustomization.
    let mut sts_by_ks: HashMap<String, Vec<WorkloadInfo>> = HashMap::new();
    for obj in sts_list.items {
        let parent = flux_parent_label(&obj);
        if parent.is_empty() { continue; }
        sts_by_ks.entry(parent).or_default().push(parse_workload(&obj));
    }

    let mut nodes = Vec::new();
    for obj in ks_list.items {
        let name = obj.metadata.name.clone().unwrap_or_default();
        let namespace = obj.metadata.namespace.clone().unwrap_or_default();
        let id = format!("{namespace}/{name}");

        let suspended = obj.data["spec"]["suspend"].as_bool().unwrap_or(false);
        let (status, message) = parse_status(&obj.data, suspended);

        let revision = obj.data["status"]["lastAppliedRevision"]
            .as_str()
            .map(str::to_string);

        let reconciled_at = obj.data["status"]["lastHandledReconcileAt"]
            .as_str()
            .map(str::to_string);

        let depends_on: Vec<String> = obj.data["spec"]["dependsOn"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|dep| {
                        let dep_name = dep["name"].as_str()?;
                        let dep_ns = dep["namespace"].as_str().unwrap_or(&namespace);
                        Some(format!("{dep_ns}/{dep_name}"))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let source = {
            let kind = obj.data["spec"]["sourceRef"]["kind"].as_str().unwrap_or("");
            let src_name = obj.data["spec"]["sourceRef"]["name"].as_str().unwrap_or("");
            let src_ns = obj.data["spec"]["sourceRef"]["namespace"]
                .as_str()
                .unwrap_or(&namespace);
            if kind.is_empty() || src_name.is_empty() {
                None
            } else {
                source_map.get(&format!("{kind}/{src_ns}/{src_name}")).cloned()
            }
        };

        let mut helm_releases = hr_by_ks.remove(&name).unwrap_or_default();
        helm_releases.sort_by(|a, b| a.name.cmp(&b.name));

        let mut deployments = deploy_by_ks.remove(&name).unwrap_or_default();
        deployments.sort_by(|a, b| a.name.cmp(&b.name));

        let mut stateful_sets = sts_by_ks.remove(&name).unwrap_or_default();
        stateful_sets.sort_by(|a, b| a.name.cmp(&b.name));

        nodes.push(KustomizationNode {
            id,
            name,
            namespace,
            status,
            message,
            revision,
            reconciled_at,
            depends_on,
            source,
            helm_releases,
            deployments,
            stateful_sets,
        });
    }

    Ok(FluxGraphData { nodes })
}

#[cfg(feature = "ssr")]
fn flux_parent_label(obj: &kube::api::DynamicObject) -> String {
    obj.metadata
        .labels
        .as_ref()
        .and_then(|l| l.get("kustomize.toolkit.fluxcd.io/name"))
        .cloned()
        .unwrap_or_default()
}

#[cfg(feature = "ssr")]
fn parse_status(data: &serde_json::Value, suspended: bool) -> (ReadyStatus, Option<String>) {
    if suspended {
        return (ReadyStatus::Suspended, Some("Suspended".into()));
    }
    let ready = data["status"]["conditions"]
        .as_array()
        .and_then(|arr| arr.iter().find(|c| c["type"].as_str() == Some("Ready")));
    match ready {
        Some(c) => {
            let msg = c["message"].as_str().map(str::to_string);
            let status = match c["status"].as_str().unwrap_or("Unknown") {
                "True" => ReadyStatus::Ready,
                "False" => ReadyStatus::Failed,
                _ => ReadyStatus::Unknown,
            };
            (status, msg)
        }
        None => (ReadyStatus::Unknown, None),
    }
}

#[cfg(feature = "ssr")]
fn parse_workload(obj: &kube::api::DynamicObject) -> WorkloadInfo {
    let name = obj.metadata.name.clone().unwrap_or_default();
    let desired = obj.data["spec"]["replicas"].as_u64().unwrap_or(1) as u32;
    let ready = obj.data["status"]["readyReplicas"].as_u64().unwrap_or(0) as u32;
    WorkloadInfo { name, ready, desired }
}

#[cfg(feature = "ssr")]
fn parse_helm_release(obj: &kube::api::DynamicObject) -> HelmReleaseInfo {
    let name = obj.metadata.name.clone().unwrap_or_default();
    let suspended = obj.data["spec"]["suspend"].as_bool().unwrap_or(false);
    let (status, message) = parse_status(&obj.data, suspended);
    let chart = obj.data["spec"]["chart"]["spec"]["chart"]
        .as_str()
        .map(str::to_string);
    let revision = obj.data["status"]["lastAppliedRevision"]
        .as_str()
        .map(str::to_string);
    HelmReleaseInfo { name, chart, status, revision, message }
}

#[cfg(feature = "ssr")]
fn parse_source_info(obj: &kube::api::DynamicObject, kind: &str) -> SourceInfo {
    let name = obj.metadata.name.clone().unwrap_or_default();
    let suspended = obj.data["spec"]["suspend"].as_bool().unwrap_or(false);
    let (status, _) = parse_status(&obj.data, suspended);
    let revision = obj.data["status"]["artifact"]["revision"]
        .as_str()
        .map(str::to_string);
    let url = obj.data["spec"]["url"].as_str().map(str::to_string);
    SourceInfo { name, kind: kind.to_string(), status, revision, url }
}

// ── Layout (pure Rust, runs on both SSR and WASM) ────────────────────────────

const NODE_W: f64 = 114.0;
const NODE_H: f64 = 36.0;
const LAYER_GAP: f64 = 88.0;
const SVG_W: f64 = 680.0;
const SVG_PAD_X: f64 = 40.0;
const SVG_PAD_Y: f64 = 28.0;

fn assign_layers(nodes: &[KustomizationNode]) -> HashMap<String, usize> {
    let mut layers: HashMap<String, usize> = HashMap::new();
    let mut changed = true;
    let mut guard = 0;
    while changed && guard < 200 {
        changed = false;
        guard += 1;
        for node in nodes {
            let parent_max = node
                .depends_on
                .iter()
                .filter_map(|dep| layers.get(dep).copied())
                .max();
            let new_layer = parent_max.map(|l| l + 1).unwrap_or(0);
            let e = layers.entry(node.id.clone()).or_insert(0);
            if new_layer > *e {
                *e = new_layer;
                changed = true;
            }
        }
    }
    layers
}

fn layout_positions(nodes: &[KustomizationNode]) -> HashMap<String, (f64, f64)> {
    let layers = assign_layers(nodes);
    let max_layer = layers.values().copied().max().unwrap_or(0);

    let mut by_layer: Vec<Vec<&KustomizationNode>> = vec![vec![]; max_layer + 1];
    for node in nodes {
        let l = layers.get(&node.id).copied().unwrap_or(0);
        by_layer[l].push(node);
    }
    for layer in &mut by_layer {
        layer.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let usable_w = SVG_W - 2.0 * SVG_PAD_X;
    let mut positions = HashMap::new();
    for (layer_idx, layer_nodes) in by_layer.iter().enumerate() {
        let n = layer_nodes.len();
        let y = SVG_PAD_Y + (layer_idx as f64) * LAYER_GAP + NODE_H / 2.0;
        for (i, node) in layer_nodes.iter().enumerate() {
            let x = if n == 1 {
                SVG_W / 2.0
            } else {
                SVG_PAD_X + (i as f64) * usable_w / (n as f64 - 1.0)
            };
            positions.insert(node.id.clone(), (x, y));
        }
    }
    positions
}

fn svg_height(nodes: &[KustomizationNode]) -> f64 {
    let layers = assign_layers(nodes);
    let max_layer = layers.values().copied().max().unwrap_or(0);
    SVG_PAD_Y * 2.0 + (max_layer as f64) * LAYER_GAP + NODE_H
}

// Short SHA from a Flux revision string like "main@sha1:abc1234..."
fn short_rev(rev: &str) -> String {
    rev.split(':')
        .next_back()
        .map(|s| s.chars().take(7).collect())
        .unwrap_or_else(|| rev.chars().take(7).collect())
}

// Compact timestamp: "2026-02-22T19:04:18Z" → "Feb 22 19:04"
fn format_ts(ts: &str) -> String {
    let date_time: Vec<&str> = ts.splitn(2, 'T').collect();
    if date_time.len() != 2 { return ts.chars().take(16).collect(); }
    let date_parts: Vec<&str> = date_time[0].splitn(3, '-').collect();
    let time = &date_time[1][..date_time[1].len().min(5)];
    if date_parts.len() == 3 {
        let month = match date_parts[1] {
            "01" => "Jan", "02" => "Feb", "03" => "Mar", "04" => "Apr",
            "05" => "May", "06" => "Jun", "07" => "Jul", "08" => "Aug",
            "09" => "Sep", "10" => "Oct", "11" => "Nov", "12" => "Dec",
            _ => date_parts[1],
        };
        format!("{} {} {}", month, date_parts[2], time)
    } else {
        format!("{} {}", date_time[0], time)
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn FluxGraphView() -> impl IntoView {
    let data = Resource::new(|| (), |_| get_flux_graph());
    let selected: RwSignal<Option<String>> = RwSignal::new(None);

    view! {
        <div class="w-full">
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-xl font-bold text-charcoal">"GitOps"</h2>
                <span class="text-xs text-charcoal-lighter font-mono">"Flux CD · live"</span>
            </div>

            <Suspense fallback=|| view! {
                <div class="text-center text-charcoal-lighter py-12 text-sm">
                    "Connecting to cluster..."
                </div>
            }>
                {move || data.get().map(|result| match result {
                    Err(_) => view! {
                        <div class="text-center text-charcoal-lighter py-12 text-sm">
                            "Cluster unavailable"
                        </div>
                    }.into_any(),

                    Ok(graph) if graph.nodes.is_empty() => view! {
                        <div class="text-center text-charcoal-lighter py-12 text-sm">
                            "No Flux resources found"
                        </div>
                    }.into_any(),

                    Ok(graph) => {
                        let positions = layout_positions(&graph.nodes);
                        let h = svg_height(&graph.nodes);

                        let edges: Vec<(String, String)> = graph.nodes.iter()
                            .flat_map(|n| n.depends_on.iter().map(|dep| (dep.clone(), n.id.clone())))
                            .collect();

                        let nodes_svg = graph.nodes.clone();
                        let nodes_detail = graph.nodes.clone();

                        view! {
                            <div class="flex flex-col gap-4">
                                // ── DAG ──────────────────────────────────────
                                <div class="bg-surface border border-border rounded-lg overflow-hidden">
                                    <svg
                                        viewBox={format!("0 0 {SVG_W} {h:.1}")}
                                        class="w-full"
                                        style={format!("height: {h:.0}px; max-height: 380px;")}
                                        xmlns="http://www.w3.org/2000/svg"
                                    >
                                        <defs>
                                            <marker
                                                id="arr"
                                                viewBox="0 0 8 8"
                                                refX="7" refY="4"
                                                markerWidth="5" markerHeight="5"
                                                orient="auto-start-reverse"
                                            >
                                                <path d="M 0 0 L 8 4 L 0 8 z" fill="#475569"/>
                                            </marker>
                                        </defs>

                                        // Edges
                                        {edges.iter().filter_map(|(from, to)| {
                                            let (fx, fy) = *positions.get(from)?;
                                            let (tx, ty) = *positions.get(to)?;
                                            let y0 = fy + NODE_H / 2.0;
                                            let y3 = ty - NODE_H / 2.0;
                                            let cy = (y3 - y0) / 2.5;
                                            let d = format!(
                                                "M {fx:.1},{y0:.1} C {fx:.1},{:.1} {tx:.1},{:.1} {tx:.1},{y3:.1}",
                                                y0 + cy, y3 - cy
                                            );
                                            Some(view! {
                                                <path
                                                    d={d}
                                                    fill="none"
                                                    stroke="#475569"
                                                    stroke-width="1.5"
                                                    marker-end="url(#arr)"
                                                />
                                            })
                                        }).collect::<Vec<_>>()}

                                        // Kustomization nodes
                                        {nodes_svg.iter().filter_map(|node| {
                                            let (cx, cy) = *positions.get(&node.id)?;
                                            let x = cx - NODE_W / 2.0;
                                            let y = cy - NODE_H / 2.0;
                                            let dot_color = node.status.dot_color();
                                            let node_id = node.id.clone();
                                            let node_id_fill = node.id.clone();
                                            let node_id_stroke = node.id.clone();
                                            let name = node.name.clone();

                                            Some(view! {
                                                <g
                                                    class="cursor-pointer"
                                                    on:click=move |_| {
                                                        let id = node_id.clone();
                                                        selected.update(|s| {
                                                            *s = if s.as_deref() == Some(&id) { None } else { Some(id) };
                                                        });
                                                    }
                                                >
                                                    <rect
                                                        x={format!("{x:.1}")}
                                                        y={format!("{y:.1}")}
                                                        width={format!("{NODE_W}")}
                                                        height={format!("{NODE_H}")}
                                                        rx="6"
                                                        fill={move || {
                                                            if selected.get().as_deref() == Some(node_id_fill.as_str()) {
                                                                "#1e3a5f"
                                                            } else {
                                                                "#0f172a"
                                                            }
                                                        }}
                                                        stroke={move || {
                                                            if selected.get().as_deref() == Some(node_id_stroke.as_str()) {
                                                                "#3b82f6"
                                                            } else {
                                                                "#334155"
                                                            }
                                                        }}
                                                        stroke-width="1"
                                                    />
                                                    <circle
                                                        cx={format!("{:.1}", x + 12.0)}
                                                        cy={format!("{cy:.1}")}
                                                        r="4"
                                                        fill={dot_color}
                                                    />
                                                    <text
                                                        x={format!("{:.1}", x + 23.0)}
                                                        y={format!("{:.1}", cy + 4.0)}
                                                        font-size="11"
                                                        font-family="ui-monospace, monospace"
                                                        fill="#94a3b8"
                                                    >
                                                        {name}
                                                    </text>
                                                </g>
                                            })
                                        }).collect::<Vec<_>>()}
                                    </svg>
                                </div>

                                // ── Detail panel ─────────────────────────────
                                {move || {
                                    let id = selected.get()?;
                                    let node = nodes_detail.iter().find(|n| n.id == id)?.clone();
                                    Some(view! {
                                        <div class="bg-surface border border-border rounded-lg p-4 space-y-3">
                                            // Header
                                            <div class="flex items-center justify-between gap-4">
                                                <div class="flex items-center gap-2 min-w-0">
                                                    <div
                                                        class="w-2 h-2 rounded-full flex-shrink-0"
                                                        style={format!("background:{}", node.status.dot_color())}
                                                    />
                                                    <span class="font-mono text-sm text-charcoal truncate">
                                                        {node.name.clone()}
                                                    </span>
                                                    <span class="text-xs text-charcoal-lighter flex-shrink-0">
                                                        {node.status.label()}
                                                    </span>
                                                </div>
                                                <div class="flex items-center gap-3 flex-shrink-0">
                                                    {node.reconciled_at.as_ref().map(|t| view! {
                                                        <span class="font-mono text-xs text-charcoal-lighter" title="last reconciled">
                                                            {format_ts(t)}
                                                        </span>
                                                    })}
                                                    {node.revision.as_ref().map(|r| view! {
                                                        <span class="font-mono text-xs text-charcoal-lighter">
                                                            {short_rev(r)}
                                                        </span>
                                                    })}
                                                </div>
                                            </div>

                                            // Status message
                                            {node.message.as_ref().map(|m| view! {
                                                <p class="text-xs text-charcoal-lighter font-mono truncate">
                                                    {m.clone()}
                                                </p>
                                            })}

                                            // Source
                                            {node.source.as_ref().map(|src| {
                                                let kind_short = if src.kind == "OCIRepository" { "oci" } else { "git" };
                                                let label = format!("{kind_short}/{}", src.name);
                                                let color = src.status.dot_color();
                                                let rev = src.revision.as_deref().map(short_rev);
                                                let url = src.url.clone();
                                                view! {
                                                    <div class="flex items-center gap-2 text-xs font-mono text-charcoal-lighter">
                                                        <div class="w-1.5 h-1.5 rounded-full flex-shrink-0" style={format!("background:{color}")}/>
                                                        <span class="text-charcoal">{label}</span>
                                                        {rev.map(|r| view! {
                                                            <span>{r}</span>
                                                        })}
                                                        {url.map(|u| view! {
                                                            <span class="truncate opacity-60">{u}</span>
                                                        })}
                                                    </div>
                                                }
                                            })}

                                            // HelmReleases
                                            {(!node.helm_releases.is_empty()).then(|| view! {
                                                <div>
                                                    <p class="text-xs text-charcoal-lighter mb-2 uppercase tracking-wider">
                                                        "HelmReleases"
                                                    </p>
                                                    <div class="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                                                        {node.helm_releases.iter().map(|hr| {
                                                            let color = hr.status.dot_color();
                                                            let name = hr.name.clone();
                                                            let chart = hr.chart.clone().unwrap_or_else(|| name.clone());
                                                            view! {
                                                                <div class="flex items-center gap-1.5 bg-gray border border-border rounded px-2 py-1.5 text-xs font-mono min-w-0">
                                                                    <div class="w-1.5 h-1.5 rounded-full flex-shrink-0" style={format!("background:{color}")}/>
                                                                    <span class="text-charcoal truncate" title={chart.clone()}>{name}</span>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            })}

                                            // Deployments
                                            {(!node.deployments.is_empty()).then(|| view! {
                                                <div>
                                                    <p class="text-xs text-charcoal-lighter mb-2 uppercase tracking-wider">
                                                        "Deployments"
                                                    </p>
                                                    <div class="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                                                        {node.deployments.iter().map(|d| {
                                                            let color = d.status().dot_color();
                                                            let name = d.name.clone();
                                                            let label = format!("{}/{}", d.ready, d.desired);
                                                            view! {
                                                                <div class="flex items-center gap-1.5 bg-gray border border-border rounded px-2 py-1.5 text-xs font-mono min-w-0">
                                                                    <div class="w-1.5 h-1.5 rounded-full flex-shrink-0" style={format!("background:{color}")}/>
                                                                    <span class="text-charcoal truncate">{name}</span>
                                                                    <span class="text-charcoal-lighter ml-auto flex-shrink-0">{label}</span>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            })}

                                            // StatefulSets
                                            {(!node.stateful_sets.is_empty()).then(|| view! {
                                                <div>
                                                    <p class="text-xs text-charcoal-lighter mb-2 uppercase tracking-wider">
                                                        "StatefulSets"
                                                    </p>
                                                    <div class="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                                                        {node.stateful_sets.iter().map(|s| {
                                                            let color = s.status().dot_color();
                                                            let name = s.name.clone();
                                                            let label = format!("{}/{}", s.ready, s.desired);
                                                            view! {
                                                                <div class="flex items-center gap-1.5 bg-gray border border-border rounded px-2 py-1.5 text-xs font-mono min-w-0">
                                                                    <div class="w-1.5 h-1.5 rounded-full flex-shrink-0" style={format!("background:{color}")}/>
                                                                    <span class="text-charcoal truncate">{name}</span>
                                                                    <span class="text-charcoal-lighter ml-auto flex-shrink-0">{label}</span>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            })}
                                        </div>
                                    })
                                }}

                                // ── Legend ───────────────────────────────────
                                <div class="flex items-center gap-4 text-xs text-charcoal-lighter">
                                    {[
                                        ("Ready", "#22c55e"),
                                        ("Failed", "#ef4444"),
                                        ("Suspended", "#f59e0b"),
                                        ("Unknown", "#6b7280"),
                                    ].iter().map(|(label, color)| view! {
                                        <div class="flex items-center gap-1">
                                            <div class="w-2 h-2 rounded-full" style={format!("background:{color}")}/>
                                            <span>{*label}</span>
                                        </div>
                                    }).collect::<Vec<_>>()}
                                    <span class="ml-auto">"click node for details"</span>
                                </div>
                            </div>
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}
