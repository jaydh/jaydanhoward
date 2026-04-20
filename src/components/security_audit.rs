use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    pub title: String,
    pub date: String,
    pub url: String,
    pub informational: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub scanned_at: String,
    pub dependency_count: u32,
    pub vulnerabilities: Vec<Advisory>,
    pub warnings: Vec<Advisory>,
    pub ignored: Vec<String>,
}

#[server(name = GetSecurityAudit, prefix = "/api", endpoint = "get_security_audit")]
pub async fn get_security_audit() -> Result<Option<AuditReport>, ServerFnError<String>> {
    use runfiles::{rlocation, Runfiles};

    let r = Runfiles::create().map_err(|e| ServerFnError::ServerError(e.to_string()))?;
    let path = rlocation!(r, "_main/assets/security-audit.json")
        .ok_or_else(|| ServerFnError::ServerError("security-audit.json not found".into()))?;

    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let scanned_at = v["scanned_at"].as_str().unwrap_or("unknown").to_string();
    let dependency_count = v["lockfile"]["dependency-count"].as_u64().unwrap_or(0) as u32;
    let ignored: Vec<String> = v["settings"]["ignore"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut vulnerabilities = Vec::new();
    for vuln in v["vulnerabilities"]["list"].as_array().unwrap_or(&vec![]) {
        let a = &vuln["advisory"];
        vulnerabilities.push(Advisory {
            id: a["id"].as_str().unwrap_or("").to_string(),
            package: a["package"].as_str().unwrap_or("").to_string(),
            title: a["title"].as_str().unwrap_or("").to_string(),
            date: a["date"].as_str().unwrap_or("").to_string(),
            url: a["url"].as_str().unwrap_or("").to_string(),
            informational: None,
        });
    }

    let mut warnings = Vec::new();
    if let Some(warn_map) = v["warnings"].as_object() {
        for (_kind, entries) in warn_map {
            for entry in entries.as_array().unwrap_or(&vec![]) {
                let a = &entry["advisory"];
                warnings.push(Advisory {
                    id: a["id"].as_str().unwrap_or("").to_string(),
                    package: a["package"].as_str().unwrap_or("").to_string(),
                    title: a["title"].as_str().unwrap_or("").to_string(),
                    date: a["date"].as_str().unwrap_or("").to_string(),
                    url: a["url"].as_str().unwrap_or("").to_string(),
                    informational: a["informational"].as_str().map(String::from),
                });
            }
        }
    }

    Ok(Some(AuditReport {
        scanned_at,
        dependency_count,
        vulnerabilities,
        warnings,
        ignored,
    }))
}

#[component]
pub fn SecurityAudit() -> impl IntoView {
    let report = Resource::new(|| (), |_| get_security_audit());

    view! {
        <div class="flex flex-col gap-4 w-full">
            <Suspense fallback=|| view! { <div class="text-charcoal-light text-sm">"Loading audit..."</div> }>
                {move || {
                    report.get().map(|res| match res {
                        Err(_) => view! {
                            <div class="text-charcoal-light text-sm">"Audit unavailable"</div>
                        }.into_any(),
                        Ok(None) => view! {
                            <div class="text-charcoal-light text-sm">"No audit report yet — runs after each deploy."</div>
                        }.into_any(),
                        Ok(Some(r)) => view! {
                            <AuditReportView report=r />
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AuditReportView(report: AuditReport) -> impl IntoView {
    let all_clear = report.vulnerabilities.is_empty();
    let total_issues = report.vulnerabilities.len() + report.warnings.len();

    view! {
        <div class="flex flex-col gap-4 w-full">
            <div class="flex items-center gap-3 text-sm text-charcoal-light">
                <span class={if all_clear { "text-green-500 font-medium" } else { "text-red-500 font-medium" }}>
                    {if all_clear { "✓ Clean" } else { "✗ Vulnerabilities found" }}
                </span>
                <span>"·"</span>
                <span>{report.dependency_count}" dependencies"</span>
                <span>"·"</span>
                <span>{total_issues}" advisories"</span>
                <span>"·"</span>
                <span class="italic">"Last checked: "{report.scanned_at}</span>
            </div>

            {(!report.vulnerabilities.is_empty()).then(|| view! {
                <AdvisoryTable
                    title="Vulnerabilities"
                    items=report.vulnerabilities.clone()
                    row_class="text-red-500"
                />
            })}

            {(!report.warnings.is_empty()).then(|| view! {
                <AdvisoryTable
                    title="Warnings (ignored)"
                    items=report.warnings.clone()
                    row_class="text-charcoal-light"
                />
            })}
        </div>
    }
}

#[component]
fn AdvisoryTable(title: &'static str, items: Vec<Advisory>, row_class: &'static str) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-2">
            <h3 class="text-sm font-medium text-charcoal">{title}</h3>
            <div class="overflow-x-auto">
                <table class="w-full text-sm border-collapse">
                    <thead>
                        <tr class="border-b border-border text-left text-charcoal-light">
                            <th class="pb-2 pr-4 font-medium">"ID"</th>
                            <th class="pb-2 pr-4 font-medium">"Crate"</th>
                            <th class="pb-2 pr-4 font-medium">"Title"</th>
                            <th class="pb-2 font-medium">"Date"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {items.into_iter().map(|a| {
                            let url = a.url.clone();
                            view! {
                                <tr class="border-b border-border last:border-0">
                                    <td class="py-2 pr-4">
                                        <a
                                            href=url
                                            target="_blank"
                                            rel="noreferrer"
                                            class={format!("hover:underline font-mono text-xs {row_class}")}
                                        >
                                            {a.id}
                                        </a>
                                    </td>
                                    <td class="py-2 pr-4 font-mono text-xs text-charcoal">{a.package}</td>
                                    <td class="py-2 pr-4 text-charcoal">{a.title}</td>
                                    <td class="py-2 text-charcoal-light whitespace-nowrap">{a.date}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}
