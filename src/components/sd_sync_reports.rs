// Bazel clippy doesn't trace through Leptos view! macros or #[component]-generated
// props structs, producing false dead_code positives throughout this file.
#![allow(dead_code)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRecord {
    pub name: String,
    pub size_bytes: u64,
    pub outcome: String,
    pub dest_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncReport {
    pub device: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_seconds: f64,
    pub files_copied: u64,
    pub files_skipped: u64,
    pub bytes_copied: u64,
    pub errors: u64,
    pub files: Vec<FileRecord>,
}

const SD_SYNC_BASE: &str = "http://sd-sync.media.svc.cluster.local:9105";

#[server(name = FetchSdSyncReportList, prefix = "/api", endpoint = "sd_sync_reports")]
pub async fn fetch_report_list() -> Result<Vec<String>, ServerFnError<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let names: Vec<String> = client
        .get(format!("{SD_SYNC_BASE}/reports"))
        .send()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?
        .json()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(names.into_iter().take(5).collect())
}

#[server(name = FetchSdSyncReport, prefix = "/api", endpoint = "sd_sync_report")]
pub async fn fetch_report(name: String) -> Result<SyncReport, ServerFnError<String>> {
    // Guard against traversal on the server side too
    if name.contains('/') || name.contains("..") || !name.ends_with(".json") {
        return Err(ServerFnError::ServerError("invalid name".into()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let report: SyncReport = client
        .get(format!("{SD_SYNC_BASE}/reports/{name}"))
        .send()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?
        .json()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(report)
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{bytes} B")
    }
}

fn fmt_ts(ts: i64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let t = UNIX_EPOCH + Duration::from_secs(ts as u64);
    let secs = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

#[component]
fn ReportRow(name: String) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let name_display = name.trim_end_matches(".json").to_string();
    let name_for_resource = name.clone();
    let report_resource = Resource::new(
        move || (expanded.get(), name_for_resource.clone()),
        |(expanded, name)| async move {
            if expanded {
                fetch_report(name).await.ok()
            } else {
                None
            }
        },
    );

    view! {
        <div class="border border-border rounded-lg overflow-hidden">
            <button
                class="w-full flex items-center justify-between px-4 py-3 bg-surface hover:bg-border/20 transition-colors text-left"
                on:click=move |_| set_expanded.update(|v| *v = !*v)
            >
                <span class="font-mono text-sm text-charcoal">{name_display}</span>
                <span class="text-charcoal-light text-lg">{move || if expanded.get() { "−" } else { "+" }}</span>
            </button>
            {move || expanded.get().then(|| view! {
                <div class="border-t border-border p-4">
                    <Suspense fallback=move || view! {
                        <p class="text-charcoal-light text-sm">"Loading..."</p>
                    }>
                        {move || report_resource.get().map(|r| match r {
                            None => view! { <p class="text-red-500 text-sm">"Failed to load report"</p> }.into_any(),
                            Some(report) => view! {
                                <div class="flex flex-col gap-4">
                                    // Summary bar
                                    <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                                        <div class="bg-border/30 rounded p-3">
                                            <p class="text-xs text-charcoal-light">"Device"</p>
                                            <p class="font-mono text-sm text-charcoal">{report.device.clone()}</p>
                                        </div>
                                        <div class="bg-border/30 rounded p-3">
                                            <p class="text-xs text-charcoal-light">"When"</p>
                                            <p class="text-sm text-charcoal">{fmt_ts(report.started_at)}</p>
                                        </div>
                                        <div class="bg-border/30 rounded p-3">
                                            <p class="text-xs text-charcoal-light">"Copied"</p>
                                            <p class="text-sm text-charcoal">
                                                {format!("{} files · {}", report.files_copied, fmt_bytes(report.bytes_copied))}
                                            </p>
                                        </div>
                                        <div class="bg-border/30 rounded p-3">
                                            <p class="text-xs text-charcoal-light">"Skipped / Errors"</p>
                                            <p class="text-sm text-charcoal">
                                                {format!("{} / {}", report.files_skipped, report.errors)}
                                            </p>
                                        </div>
                                    </div>
                                    // File table
                                    <div class="overflow-x-auto">
                                        <table class="w-full text-xs font-mono">
                                            <thead>
                                                <tr class="border-b border-border text-charcoal-light text-left">
                                                    <th class="pb-2 pr-4">"File"</th>
                                                    <th class="pb-2 pr-4">"Size"</th>
                                                    <th class="pb-2 pr-4">"Status"</th>
                                                    <th class="pb-2">"Destination"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {report.files.iter().map(|f| {
                                                    let outcome_class = match f.outcome.as_str() {
                                                        "copied" => "text-green-600 dark:text-green-400",
                                                        "skipped" => "text-charcoal-light",
                                                        _ => "text-red-500",
                                                    };
                                                    let dest = f.dest_path
                                                        .rsplit('/')
                                                        .next()
                                                        .unwrap_or(&f.dest_path)
                                                        .to_string();
                                                    view! {
                                                        <tr class="border-b border-border/50 hover:bg-border/10">
                                                            <td class="py-1.5 pr-4 text-charcoal">{f.name.clone()}</td>
                                                            <td class="py-1.5 pr-4 text-charcoal-light">{fmt_bytes(f.size_bytes)}</td>
                                                            <td class=format!("py-1.5 pr-4 {outcome_class}")>{f.outcome.clone()}</td>
                                                            <td class="py-1.5 text-charcoal-light truncate max-w-xs">{dest}</td>
                                                        </tr>
                                                    }
                                                }).collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                </div>
                            }.into_any(),
                        })}
                    </Suspense>
                </div>
            })}
        </div>
    }
}

#[component]
pub fn SdSyncReports() -> impl IntoView {
    let list_resource = Resource::new(|| (), |_| fetch_report_list());

    view! {
        <div class="w-full flex flex-col gap-3">
            <Suspense fallback=move || view! {
                <p class="text-charcoal-light text-sm">"Loading sync reports..."</p>
            }>
                {move || list_resource.get().map(|result| match result {
                    Err(_) => view! {
                        <p class="text-charcoal-light text-sm">"Sync reports unavailable"</p>
                    }.into_any(),
                    Ok(names) if names.is_empty() => view! {
                        <p class="text-charcoal-light text-sm">"No sync reports yet"</p>
                    }.into_any(),
                    Ok(names) => names.into_iter().map(|name| view! {
                        <ReportRow name=name />
                    }).collect_view().into_any(),
                })}
            </Suspense>
        </div>
    }
}
