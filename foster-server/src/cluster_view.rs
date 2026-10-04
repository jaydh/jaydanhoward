//! Display model for the "Homelab Cluster" card: turns the nested snapshot
//! from `cluster::fetch_cluster_snapshot` into flat, preformatted fields and
//! lists that Foster markup binds directly (`fx-text`, `fx-for` + `fx-field`,
//! `fx-bind-attr`). Foster bindings read `ctx[key]` / `item[field]` only — no
//! dotted paths or formatting — so all of that happens here.
//!
//! Sparklines are SVG path data (`d`), bound with `fx-bind-attr="d=ctx:…"`
//! onto a `preserveAspectRatio="none"` viewBox, so no canvas drawing code.

use serde_json::{json, Value};

/// Sparkline viewBox sizes (the CSS stretches them to the card width).
const SPARK_W: f64 = 260.0;
const SPARK_H: f64 = 50.0;
const NET_H: f64 = 70.0;
/// Cloudflared HA connections expected when the tunnel is fully healthy.
const EXPECTED_HA_CONNS: i64 = 12;

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn i(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|x| x as i64)).unwrap_or(0)
}

/// Display text of a JSON scalar: strings unquoted, null as "".
fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// JS truthiness, matching the old `e.error ? … : …` check.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(x) => !x.is_empty(),
        _ => true,
    }
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn series(v: &Value) -> Vec<f64> {
    arr(v).iter().map(f).collect()
}

fn fmt_bytes(bytes: f64) -> String {
    if bytes >= 1_073_741_824.0 {
        format!("{:.1}G", bytes / 1_073_741_824.0)
    } else {
        format!("{:.0}M", bytes / 1_048_576.0)
    }
}

fn pct(used: f64, total: f64) -> f64 {
    if total > 0.0 { (used / total * 100.0).min(100.0) } else { 0.0 }
}

/// SVG path through `points` scaled into a `w`x`h` box against `max`, with a
/// 3-unit margin top and bottom. Empty for fewer than two points.
pub fn spark_path(points: &[f64], max: f64, w: f64, h: f64) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let max = max.max(1.0);
    let last = (points.len() - 1) as f64;
    points
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f64 / last * w;
            let y = h - (v / max) * (h - 6.0) - 3.0;
            format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { "L" })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn spark(points: &[f64]) -> String {
    spark_path(points, points.iter().cloned().fold(0.0, f64::max), SPARK_W, SPARK_H)
}

/// Build the card's display model from a `fetch_cluster_snapshot` value
/// (any missing piece renders as zero/empty, same as the old JS did).
pub fn cluster_view(snap: &Value, updated_at: &str) -> Value {
    let cluster = &snap["cluster"];
    let ceph = &snap["ceph"];
    let hist = &snap["historical"]["series"];
    let cf = &snap["cloudflared"];
    let spike = &snap["spike_config"];
    let sec = &snap["security_audit"];

    let rx = series(&hist["network_rx"]);
    let tx = series(&hist["network_tx"]);
    let net_max = rx.iter().chain(&tx).cloned().fold(0.0, f64::max);

    let nodes: Vec<Value> = arr(&snap["nodes"])
        .iter()
        .map(|n| {
            let cpu = f(&n["cpu_usage_percent"]);
            let mem_pct = pct(f(&n["memory_usage_gb"]), f(&n["memory_total_gb"]));
            json!({
                "name": s(&n["name"]),
                "cpu_label": format!("{cpu:.1}%"),
                "cpu_width": format!("{cpu:.1}%"),
                "mem_label": format!("{:.1}G", f(&n["memory_usage_gb"])),
                "mem_width": format!("{mem_pct:.1}%"),
            })
        })
        .collect();

    let pvcs: Vec<Value> = arr(&cluster["pvcs"])
        .iter()
        .map(|p| {
            let (used, cap) = (f(&p["used_bytes"]), f(&p["capacity_bytes"]));
            json!({
                "label": format!("{}/{}", s(&p["namespace"]), s(&p["name"])),
                "width": format!("{:.1}%", pct(used, cap)),
                "value": format!("{} / {}", fmt_bytes(used), fmt_bytes(cap)),
            })
        })
        .collect();

    let ceph_health = ceph["health"]
        .as_u64()
        .and_then(|h| ["OK", "WARN", "ERR"].get(h as usize).copied())
        .unwrap_or("unknown");

    let ha = i(&cf["ha_connections"]);
    let ha_label = if ha >= EXPECTED_HA_CONNS { "healthy" } else if ha > 0 { "degraded" } else { "down" };

    let list = |key: &str, row: &dyn Fn(&Value) -> Value| -> Vec<Value> { arr(&snap[key]).iter().map(row).collect() };

    json!({
        "updated_at": updated_at,
        "pod_count": i(&cluster["pod_count"]),
        "node_ready": format!("{}/{}", i(&cluster["healthy_node_count"]), i(&cluster["node_count"])),
        "spark_cpu": spark(&series(&hist["cpu"])),
        "spark_memory": spark(&series(&hist["memory"])),
        "spark_disk": spark(&series(&hist["disk"])),
        "spark_rx": spark_path(&rx, net_max, SPARK_W, NET_H),
        "spark_tx": spark_path(&tx, net_max, SPARK_W, NET_H),
        "nodes": nodes,

        "pvcs": pvcs,
        "ceph_health": ceph_health,
        "ceph_mon": format!("{}/{}", i(&ceph["mon_quorum"]), i(&ceph["mon_total"])),
        "ceph_mgr": format!("{} active, {} standby", i(&ceph["mgr_active"]), i(&ceph["mgr_standby"])),
        "ceph_osd": format!("{}/{} up", i(&ceph["osd_up"]), i(&ceph["osd_total"])),
        "ceph_pg": format!("{}/{}", i(&ceph["pg_clean"]), i(&ceph["pg_total"])),
        "ceph_pools": i(&ceph["pool_count"]),
        "ceph_data": format!("{} / {}", fmt_bytes(f(&ceph["data_used_bytes"])), fmt_bytes(f(&ceph["data_total_bytes"]))),
        "ceph_rw": format!("{}/s / {}/s", fmt_bytes(f(&ceph["read_bytes_per_sec"])), fmt_bytes(f(&ceph["write_bytes_per_sec"]))),
        "backups": list("backups", &|b| json!({
            "icon": s(&b["status_icon"]), "name": s(&b["name"]), "label": s(&b["status_label"]),
        })),

        "cf_status": format!("{ha_label} ({ha}/{EXPECTED_HA_CONNS} HA conns)"),
        "cf_rps": format!("{:.2}", f(&cf["total_req_per_sec"])),
        "cf_errors": format!("{:.3}/s", f(&cf["error_rate"])),
        "cf_by_status": arr(&cf["by_status"]).iter().map(|b| json!({
            "code": s(&b["status_code"]), "rate": format!("{:.2}/s", f(&b["req_per_sec"])),
        })).collect::<Vec<_>>(),
        "top_pods": list("top_pods", &|p| json!({
            "pod": format!("{}/{}", s(&p["namespace"]), s(&p["pod"])),
            "tx": format!("{:.1}", f(&p["tx_mbps"])),
            "rx": format!("{:.1}", f(&p["rx_mbps"])),
        })),
        "spike_threshold": format!("{:.1}x above baseline",
            spike["multiplier"].as_f64().unwrap_or(3.0)),
        "spike_floor": format!("{:.0} Mbps", spike["floor_mbps"].as_f64().unwrap_or(5.0)),
        "insights": list("network_insights", &|n| json!({
            "when": s(&n["occurred_at"]),
            "text": format!("{:.1} Mbps (baseline {:.1})", f(&n["spike_tx_mbps"]), f(&n["baseline_tx_mbps"])),
        })),

        "gitops": list("gitops", &|r| json!({
            "kind": s(&r["kind"]),
            "name": format!("{}/{}", s(&r["namespace"]), s(&r["name"])),
            "icon": s(&r["ready_icon"]),
        })),

        "alerts": list("alerts", &|a| json!({
            "name": s(&a["alertname"]),
            "detail": format!("[{}] {} — {}", s(&a["severity"]), s(&a["namespace"]), s(&a["summary"])),
        })),
        "daily_audit": list("daily_audit", &|a| json!({
            "when": s(&a["occurred_at"]),
            "significance": format!("{}/10", s(&a["significance"])),
            "summary": s(&a["summary"]),
            "findings": arr(&a["findings"]).iter()
                .map(|x| format!("{}: {}", s(&x["title"]), s(&x["detail"])))
                .collect::<Vec<_>>().join("\n"),
        })),
        "sec_status": sec["sec_status_label"].as_str().unwrap_or("No audit report yet"),
        "sec_deps": format!("{} deps", i(&sec["sec_dependency_count"])),
        "sec_advisories": format!("{} advisories", i(&sec["sec_advisory_count"])),
        "sec_scanned_at": s(&sec["sec_scanned_at"]),
        "sec_findings": arr(&sec["security_vulnerabilities"]).iter().map(|v| (v, true))
            .chain(arr(&sec["security_warnings"]).iter().map(|v| (v, false)))
            .map(|(v, vuln)| json!({
                "text": format!("{} {} — {}", s(&v["id"]), s(&v["package"]), s(&v["title"])),
                "cls": if vuln { "cluster-audit-row cluster-audit-err" } else { "cluster-audit-row cluster-audit-warn" },
            }))
            .collect::<Vec<_>>(),
        "claude_log": list("claude_log", &|e| {
            let err = truthy(&e["error"]);
            json!({
                "text": format!("{} — {} — {}{}", s(&e["occurred_at"]), s(&e["model"]), s(&e["context"]),
                    if err { " (error)" } else { "" }),
                "cls": if err { "cluster-audit-row cluster-audit-err" } else { "cluster-audit-row" },
            })
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spark_path_scales_into_box() {
        assert_eq!(spark_path(&[0.0, 10.0], 10.0, 260.0, 50.0), "M0.0,47.0 L260.0,3.0");
        assert_eq!(spark_path(&[5.0], 10.0, 260.0, 50.0), "");
    }

    #[test]
    fn empty_snapshot_renders_zeroes_and_empty_lists() {
        let v = cluster_view(&json!({}), "");
        assert_eq!(v["node_ready"], "0/0");
        assert_eq!(v["ceph_health"], "unknown");
        assert_eq!(v["cf_status"], "down (0/12 HA conns)");
        assert_eq!(v["spike_threshold"], "3.0x above baseline");
        assert_eq!(v["nodes"], json!([]));
        assert_eq!(v["spark_cpu"], "");
    }

    #[test]
    fn formats_like_the_old_js() {
        let v = cluster_view(&json!({
            "cluster": { "pod_count": 42, "node_count": 3, "healthy_node_count": 3,
                         "pvcs": [{ "namespace": "db", "name": "pg", "used_bytes": 536870912.0, "capacity_bytes": 2147483648.0 }] },
            "nodes": [{ "name": "n1", "cpu_usage_percent": 12.34, "memory_usage_gb": 4.0, "memory_total_gb": 16.0 }],
            "ceph": { "health": 1 },
            "cloudflared": { "ha_connections": 12, "by_status": [{ "status_code": 200, "req_per_sec": 1.234 }] },
            "claude_log": [{ "occurred_at": "t", "model": "m", "context": "c", "error": "boom" }],
        }), "12:00:00 UTC");
        assert_eq!(v["pod_count"], 42);
        assert_eq!(v["nodes"][0]["cpu_label"], "12.3%");
        assert_eq!(v["nodes"][0]["mem_width"], "25.0%");
        assert_eq!(v["pvcs"][0], json!({ "label": "db/pg", "width": "25.0%", "value": "512M / 2.0G" }));
        assert_eq!(v["ceph_health"], "WARN");
        assert_eq!(v["cf_status"], "healthy (12/12 HA conns)");
        assert_eq!(v["cf_by_status"][0], json!({ "code": "200", "rate": "1.23/s" }));
        assert_eq!(v["claude_log"][0]["text"], "t — m — c (error)");
    }
}
