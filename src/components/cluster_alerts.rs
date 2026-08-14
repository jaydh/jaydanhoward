use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FiringAlert {
    pub alertname: String,
    pub severity: String,
    pub namespace: String,
    pub summary: String,
    pub since: String,
}

#[server(name = GetClusterAlerts, prefix = "/api", endpoint = "get_cluster_alerts")]
pub async fn get_cluster_alerts() -> Result<Vec<FiringAlert>, ServerFnError<String>> {
    use crate::prometheus_client::query_prometheus;

    let data = query_prometheus(r#"ALERTS{alertstate="firing"}"#)
        .await
        .map_err(|e| ServerFnError::ServerError(format!("Prometheus query failed: {e}")))?;

    let alerts = data
        .data
        .result
        .into_iter()
        .map(|m| {
            let get = |k: &str| m.metric.get(k).cloned().unwrap_or_default();
            FiringAlert {
                alertname: get("alertname"),
                severity: get("severity"),
                namespace: get("namespace"),
                summary: get("summary"),
                since: m.value.0.to_string(),
            }
        })
        .collect();

    Ok(alerts)
}

#[component]
pub fn ClusterAlerts() -> impl IntoView {
    let alerts = Resource::new(|| (), |_| get_cluster_alerts());

    view! {
        <div class="flex flex-col gap-4 w-full">
            <Suspense fallback=|| view! { <div class="text-charcoal-light text-sm">"Checking alerts..."</div> }>
                {move || {
                    alerts.get().map(|res| match res {
                        Err(_) => view! {
                            <div class="text-charcoal-light text-sm">"Alerts unavailable"</div>
                        }.into_any(),
                        Ok(alerts) if alerts.is_empty() => view! {
                            <div class="flex items-center gap-3 text-sm text-charcoal-light">
                                <span class="text-green-500 font-medium">"✓ No alerts firing"</span>
                            </div>
                        }.into_any(),
                        Ok(alerts) => view! {
                            <AlertsTable alerts=alerts />
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AlertsTable(alerts: Vec<FiringAlert>) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-2">
            <div class="flex items-center gap-3 text-sm">
                <span class="text-red-500 font-medium">{format!("⚠ {} firing", alerts.len())}</span>
            </div>
            <div class="overflow-x-auto">
                <table class="w-full text-sm border-collapse">
                    <thead>
                        <tr class="border-b border-border text-left text-charcoal-light">
                            <th class="pb-2 pr-4 font-medium">"Alert"</th>
                            <th class="pb-2 pr-4 font-medium">"Severity"</th>
                            <th class="pb-2 pr-4 font-medium">"Namespace"</th>
                            <th class="pb-2 font-medium">"Summary"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {alerts.into_iter().map(|a| {
                            let row_class = if a.severity == "critical" { "text-red-500" } else { "text-amber-500" };
                            view! {
                                <tr class="border-b border-border last:border-0">
                                    <td class={format!("py-2 pr-4 font-mono text-xs {row_class}")}>{a.alertname}</td>
                                    <td class="py-2 pr-4 text-charcoal-light">{a.severity}</td>
                                    <td class="py-2 pr-4 font-mono text-xs text-charcoal">{a.namespace}</td>
                                    <td class="py-2 text-charcoal-light">{a.summary}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}
