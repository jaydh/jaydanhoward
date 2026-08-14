use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub occurred_at: String,
    pub summary: String,
    pub significance: i16,
    pub findings: Vec<Finding>,
}

#[server(name = GetClusterAudits, prefix = "/api", endpoint = "get_cluster_audits")]
pub async fn get_cluster_audits() -> Result<Vec<AuditEntry>, ServerFnError<String>> {
    use axum::extract::Extension;
    use leptos_axum::extract;
    use std::sync::Arc;

    let Extension(pool): Extension<Option<Arc<sqlx::PgPool>>> = extract()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let Some(pool) = pool else { return Ok(vec![]) };

    let rows = crate::db::get_recent_cluster_audits(&pool, 10)
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| AuditEntry {
            occurred_at: r.occurred_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            summary: r.summary,
            significance: r.significance,
            findings: serde_json::from_value(r.findings).unwrap_or_default(),
        })
        .collect())
}

#[component]
pub fn ClusterAuditLog() -> impl IntoView {
    let audits = Resource::new(|| (), |_| get_cluster_audits());

    view! {
        <div class="flex flex-col gap-4 w-full">
            <Suspense fallback=|| view! { <div class="text-charcoal-light text-sm">"Loading audit history..."</div> }>
                {move || {
                    audits.get().map(|res| match res {
                        Err(_) => view! {
                            <div class="text-charcoal-light text-sm">"Audit log unavailable"</div>
                        }.into_any(),
                        Ok(entries) if entries.is_empty() => view! {
                            <div class="text-charcoal-light text-sm">"No audit runs yet — runs daily once Prometheus and an Anthropic key are configured."</div>
                        }.into_any(),
                        Ok(entries) => view! {
                            <AuditEntries entries=entries />
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AuditEntries(entries: Vec<AuditEntry>) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-3 w-full">
            {entries.into_iter().map(|e| {
                let color = if e.significance >= 7 {
                    "text-red-500"
                } else if e.significance >= 4 {
                    "text-amber-500"
                } else {
                    "text-green-500"
                };
                view! {
                    <div class="flex flex-col gap-1 border-b border-border last:border-0 pb-3 last:pb-0">
                        <div class="flex items-center gap-3 text-sm">
                            <span class={format!("font-medium {color}")}>{format!("{}/10", e.significance)}</span>
                            <span class="text-charcoal-light italic">{e.occurred_at}</span>
                        </div>
                        <p class="text-sm text-charcoal">{e.summary}</p>
                        {(!e.findings.is_empty()).then(|| view! {
                            <ul class="text-xs text-charcoal-light list-disc list-inside pl-2">
                                {e.findings.into_iter().map(|f| view! {
                                    <li><span class="font-medium text-charcoal">{f.title}</span>": "{f.detail}</li>
                                }).collect_view()}
                            </ul>
                        })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}
