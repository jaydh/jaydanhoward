// Real server-push "Homelab Cluster" panel — consumes the genuine SSE
// stream at /api/metrics/stream (src/cluster.rs::metrics_stream, ported
// from the real site's routes/metrics_stream.rs), one tick per second.
// This is plain client-side state (tab selection is per-visitor UI state,
// the data itself is one shared homelab, not per-visitor), not a Foster
// machine — same reasoning as theme/life/pathfinding/photography.

function surfaceColor() {
  // --surface/--accent are set on .page (overridden by html.dark .page);
  // document.body is an ancestor of .page, not a descendant, so reading
  // off document.body misses the dark-mode override.
  return getComputedStyle(document.querySelector('.page') || document.body);
}

function fmtBytes(bytes) {
  bytes = Number(bytes) || 0;
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + 'G';
  return (bytes / 1048576).toFixed(0) + 'M';
}

function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]));
}

function drawSparkline(canvas, series, color) {
  if (!canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth || 260;
  const h = canvas.clientHeight || 50;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = surfaceColor().getPropertyValue('--surface') || '#fff';
  ctx.fillRect(0, 0, w, h);
  if (!series || series.length < 2) return;
  const max = Math.max(...series, 1);
  ctx.beginPath();
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  series.forEach((v, i) => {
    const x = (i / (series.length - 1)) * w;
    const y = h - (v / max) * (h - 6) - 3;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();
}

function drawNetworkSparkline(canvas, rx, tx) {
  if (!canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth || 260;
  const h = canvas.clientHeight || 70;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = surfaceColor().getPropertyValue('--surface') || '#fff';
  ctx.fillRect(0, 0, w, h);
  const drawLine = (series, color) => {
    if (!series || series.length < 2) return;
    const max = Math.max(...rx, ...tx, 1);
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    series.forEach((v, i) => {
      const x = (i / (series.length - 1)) * w;
      const y = h - (v / max) * (h - 6) - 3;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
  };
  drawLine(rx, '#3b82f6');
  drawLine(tx, '#f59e0b');
}

export function initCluster() {
  const widget = document.getElementById('cluster-widget');
  if (!widget) return;

  const tabs = widget.querySelectorAll('.cluster-tab');
  tabs.forEach((tab) => {
    tab.addEventListener('click', () => {
      tabs.forEach((t) => t.classList.remove('active'));
      widget.querySelectorAll('.cluster-tabpanel').forEach((p) => p.classList.remove('active'));
      tab.classList.add('active');
      document.getElementById(`cluster-tab-${tab.dataset.tab}`)?.classList.add('active');
    });
  });

  const errorEl = document.getElementById('cluster-error');
  const lastRefreshEl = document.getElementById('cluster-last-refresh');

  function render(snapshot) {
    const cluster = snapshot.cluster || {};
    const nodes = snapshot.nodes || [];
    const ceph = snapshot.ceph || {};
    const series = (snapshot.historical && snapshot.historical.series) || {};
    const topPods = snapshot.top_pods || [];
    const cf = snapshot.cloudflared || {};
    const gitops = snapshot.gitops || [];
    const backups = snapshot.backups || [];
    const insights = snapshot.network_insights || [];
    const spikeConfig = snapshot.spike_config || {};
    const claudeLog = snapshot.claude_log || [];
    const sec = snapshot.security_audit || {};

    // ── Overview ──
    document.getElementById('cluster-pod-count').textContent = cluster.pod_count ?? 0;
    document.getElementById('cluster-node-count').textContent =
      `${cluster.healthy_node_count ?? 0}/${cluster.node_count ?? 0}`;
    drawSparkline(document.getElementById('cluster-spark-cpu'), series.cpu, '#ef4444');
    drawSparkline(document.getElementById('cluster-spark-memory'), series.memory, '#3b82f6');
    drawSparkline(document.getElementById('cluster-spark-disk'), series.disk, '#8b5cf6');
    drawNetworkSparkline(document.getElementById('cluster-spark-network'), series.network_rx, series.network_tx);

    document.getElementById('cluster-nodes').innerHTML = nodes.map((n) => {
      const memPct = n.memory_total_gb > 0 ? Math.min(100, (n.memory_usage_gb / n.memory_total_gb) * 100) : 0;
      return `
        <div class="cluster-box" style="max-height:none">
          <h3>${escapeHtml(n.name)}</h3>
          <div class="cluster-bar-row"><span class="cluster-bar-label">CPU</span>
            <div class="cluster-bar-track"><div class="cluster-bar-fill" style="width:${(n.cpu_usage_percent || 0).toFixed(1)}%"></div></div>
            <span class="cluster-bar-val">${(n.cpu_usage_percent || 0).toFixed(1)}%</span></div>
          <div class="cluster-bar-row"><span class="cluster-bar-label">Mem</span>
            <div class="cluster-bar-track"><div class="cluster-bar-fill" style="width:${memPct.toFixed(1)}%"></div></div>
            <span class="cluster-bar-val">${(n.memory_usage_gb || 0).toFixed(1)}G</span></div>
        </div>`;
    }).join('');

    // ── Storage ──
    document.getElementById('cluster-pvcs').innerHTML = (cluster.pvcs || []).map((p) => {
      const pct = p.capacity_bytes > 0 ? Math.min(100, (p.used_bytes / p.capacity_bytes) * 100) : 0;
      return `
        <div class="cluster-bar-row">
          <span class="cluster-bar-label" title="${escapeHtml(p.namespace)}/${escapeHtml(p.name)}">${escapeHtml(p.namespace)}/${escapeHtml(p.name)}</span>
          <div class="cluster-bar-track"><div class="cluster-bar-fill" style="width:${pct.toFixed(1)}%"></div></div>
          <span class="cluster-bar-val">${fmtBytes(p.used_bytes)} / ${fmtBytes(p.capacity_bytes)}</span>
        </div>`;
    }).join('') || '<p style="color:var(--text-light);font-size:0.8rem">No PVC data yet.</p>';

    const healthLabel = ['OK', 'WARN', 'ERR'][ceph.health] ?? 'unknown';
    document.getElementById('cluster-ceph').innerHTML = `
      <div class="visit-row"><span>health</span><span>${healthLabel}</span></div>
      <div class="visit-row"><span>mon quorum</span><span>${ceph.mon_quorum ?? 0}/${ceph.mon_total ?? 0}</span></div>
      <div class="visit-row"><span>mgr</span><span>${ceph.mgr_active ?? 0} active, ${ceph.mgr_standby ?? 0} standby</span></div>
      <div class="visit-row"><span>osd</span><span>${ceph.osd_up ?? 0}/${ceph.osd_total ?? 0} up</span></div>
      <div class="visit-row"><span>pg clean</span><span>${ceph.pg_clean ?? 0}/${ceph.pg_total ?? 0}</span></div>
      <div class="visit-row"><span>pools</span><span>${ceph.pool_count ?? 0}</span></div>
      <div class="visit-row"><span>data used</span><span>${fmtBytes(ceph.data_used_bytes)} / ${fmtBytes(ceph.data_total_bytes)}</span></div>
      <div class="visit-row"><span>read/write</span><span>${fmtBytes(ceph.read_bytes_per_sec)}/s / ${fmtBytes(ceph.write_bytes_per_sec)}/s</span></div>
    `;

    document.getElementById('cluster-backups').innerHTML = backups.map((b) => `
      <div class="backup-row">
        <span>${escapeHtml(b.status_icon)}</span>
        <span class="flux-name">${escapeHtml(b.name)}</span>
        <span class="flux-kind">${escapeHtml(b.status_label)}</span>
      </div>`).join('') || '<p style="color:var(--text-light);font-size:0.8rem">No backup Job data.</p>';

    // ── Network ──
    const expected = 12;
    const haLabel = cf.ha_connections >= expected ? 'healthy' : cf.ha_connections > 0 ? 'degraded' : 'down';
    document.getElementById('cluster-cloudflared').innerHTML = `
      <div class="visit-row"><span>status</span><span>${haLabel} (${cf.ha_connections ?? 0}/${expected} HA conns)</span></div>
      <div class="visit-row"><span>req/s (5m avg)</span><span>${(cf.total_req_per_sec ?? 0).toFixed(2)}</span></div>
      <div class="visit-row"><span>error rate</span><span>${(cf.error_rate ?? 0).toFixed(3)}/s</span></div>
      ${(cf.by_status || []).map((s) => `<div class="visit-row"><span>${escapeHtml(s.status_code)}</span><span>${s.req_per_sec.toFixed(2)}/s</span></div>`).join('')}
    `;

    document.getElementById('cluster-top-pods').innerHTML = topPods.length ? `
      <table class="cluster-table">
        <tr><th>pod</th><th class="num">tx Mbps</th><th class="num">rx Mbps</th></tr>
        ${topPods.map((p) => `<tr><td>${escapeHtml(p.namespace)}/${escapeHtml(p.pod)}</td><td class="num">${p.tx_mbps.toFixed(1)}</td><td class="num">${p.rx_mbps.toFixed(1)}</td></tr>`).join('')}
      </table>` : '<p style="color:var(--text-light);font-size:0.8rem">No pod traffic data.</p>';

    document.getElementById('cluster-spike-config').innerHTML = `
      <div class="visit-row"><span>threshold</span><span>${(spikeConfig.multiplier ?? 3).toFixed(1)}x above baseline</span></div>
      <div class="visit-row"><span>floor</span><span>${(spikeConfig.floor_mbps ?? 5).toFixed(0)} Mbps</span></div>
    `;

    document.getElementById('cluster-insights').innerHTML = insights.map((i) => `
      <div class="visit-row">
        <span>${escapeHtml(i.occurred_at)}</span>
        <span>${i.spike_tx_mbps.toFixed(1)} Mbps (baseline ${i.baseline_tx_mbps.toFixed(1)})</span>
      </div>`).join('') || '<p style="color:var(--text-light);font-size:0.8rem">No spikes recorded.</p>';

    // ── GitOps ──
    document.getElementById('cluster-gitops').innerHTML = gitops.map((r) => `
      <div class="flux-row">
        <span class="flux-kind">${escapeHtml(r.kind)}</span>
        <span class="flux-name">${escapeHtml(r.namespace)}/${escapeHtml(r.name)}</span>
        <span>${escapeHtml(r.ready_icon)}</span>
      </div>`).join('') || '<p style="color:var(--text-light);font-size:0.8rem">No Flux resources found.</p>';

    // ── AI Audit ──
    document.getElementById('cluster-security-audit').innerHTML = `
      <div class="visit-row">
        <span>${escapeHtml(sec.sec_status_label ?? 'No audit report yet')}</span>
        <span>${sec.sec_dependency_count ?? 0} deps</span>
        <span>${sec.sec_advisory_count ?? 0} advisories</span>
      </div>
      <div class="visit-row"><span>last scan</span><span>${escapeHtml(sec.sec_scanned_at ?? '')}</span></div>
      ${(sec.security_vulnerabilities || []).map((v) => `<div class="cluster-audit-row cluster-audit-err">${escapeHtml(v.id)} ${escapeHtml(v.package)} — ${escapeHtml(v.title)}</div>`).join('')}
      ${(sec.security_warnings || []).map((v) => `<div class="cluster-audit-row" style="opacity:0.7">${escapeHtml(v.id)} ${escapeHtml(v.package)} — ${escapeHtml(v.title)}</div>`).join('')}
    `;

    document.getElementById('cluster-claude-log').innerHTML = claudeLog.map((e) => `
      <div class="cluster-audit-row${e.error ? ' cluster-audit-err' : ''}">
        ${escapeHtml(e.occurred_at)} — ${escapeHtml(e.model)} — ${escapeHtml(e.context)}${e.error ? ' (error)' : ''}
      </div>`).join('') || '<p style="color:var(--text-light);font-size:0.8rem">No Claude API calls recorded yet.</p>';

    errorEl.style.display = 'none';
    lastRefreshEl.textContent = new Date().toLocaleTimeString();
  }

  const source = new EventSource('/api/metrics/stream');
  source.onmessage = (event) => {
    try {
      render(JSON.parse(event.data));
    } catch (e) {
      // malformed tick — skip, next one arrives in ~1s
    }
  };
  source.onerror = () => {
    errorEl.textContent = 'Connection error: reconnecting...';
    errorEl.style.display = 'block';
  };
}
