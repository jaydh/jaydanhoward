// Real per-request trace — plain fetch + DOM rendering, no Foster machine.
// This data has nothing discrete or event-driven about it (it's a snapshot
// of the request that's already happened by the time the page loads), so
// modeling it as a machine would just be overhead. Matches scroll-spy's
// answer to "does this need Foster?": not everything does.

const IATA_CITY = {
  SJC: 'San Jose, CA', LAX: 'Los Angeles, CA', SFO: 'San Francisco, CA',
  SEA: 'Seattle, WA', DEN: 'Denver, CO', DFW: 'Dallas, TX', ORD: 'Chicago, IL',
  ATL: 'Atlanta, GA', IAD: 'Ashburn, VA', EWR: 'Newark, NJ', MIA: 'Miami, FL',
  LHR: 'London, UK', AMS: 'Amsterdam, NL', FRA: 'Frankfurt, DE', CDG: 'Paris, FR',
  MAD: 'Madrid, ES', MXP: 'Milan, IT', ARN: 'Stockholm, SE', SIN: 'Singapore',
  NRT: 'Tokyo, JP', HKG: 'Hong Kong', SYD: 'Sydney, AU', GRU: 'São Paulo, BR',
  YYZ: 'Toronto, CA',
};

function summarizeUa(ua) {
  if (!ua) return null;
  let browser = 'Unknown';
  if (ua.includes('Edg/')) browser = 'Edge';
  else if (ua.includes('Chrome/')) browser = 'Chrome';
  else if (ua.includes('Firefox/')) browser = 'Firefox';
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari';
  else if (ua.includes('curl/')) browser = 'curl';

  let os = 'Unknown';
  if (ua.includes('Windows')) os = 'Windows';
  else if (ua.includes('iPhone') || ua.includes('iPad')) os = 'iOS';
  else if (ua.includes('Android')) os = 'Android';
  else if (ua.includes('Mac OS X')) os = 'macOS';
  else if (ua.includes('Linux')) os = 'Linux';

  return `${browser} · ${os}`;
}

function card(label, rows) {
  const items = rows.filter(([, v]) => v != null && v !== '');
  return `
    <div class="trace-card">
      <p class="trace-label">${label}</p>
      <div class="trace-rows">
        ${items.map(([k, v]) => `
          <div class="trace-row">
            <span class="trace-key">${k}</span>
            <span class="trace-val">${v}</span>
          </div>
        `).join('')}
      </div>
    </div>`;
}

function hop(label) {
  return `
    <div class="trace-hop">
      <div class="trace-hop-line"></div>
      <span class="trace-hop-label">${label}</span>
      <div class="trace-hop-line"></div>
    </div>`;
}

export async function initRequestTrace() {
  const container = document.getElementById('request-trace-container');
  const button = document.getElementById('request-trace-refresh');
  if (!container) return;

  async function load() {
    container.innerHTML = '<div class="trace-loading">tracing your request…</div>';
    let data;
    try {
      const res = await fetch('/api/request-trace');
      data = await res.json();
    } catch (e) {
      container.innerHTML = '<div class="trace-error">Failed to trace request.</div>';
      return;
    }

    const cfLocation = data.cfDatacenter
      ? `${data.cfDatacenter}${IATA_CITY[data.cfDatacenter] ? ' · ' + IATA_CITY[data.cfDatacenter] : ''}`
      : null;
    const uaSummary = summarizeUa(data.userAgent);
    const geoLocation = data.geoCity && data.geoCountry
      ? `${data.geoCity}, ${data.geoCountry}`
      : data.geoCountry || data.geoCity || null;
    const protocol = data.https ? 'HTTPS · TLS 1.3' : 'HTTP';

    container.innerHTML = [
      card('You', [
        ['ip', data.ip],
        ['location', geoLocation],
        ['isp', data.geoIsp],
        ['client', uaSummary],
      ]),
      hop(protocol),
      card('Cloudflare', [
        ['pop', cfLocation],
        ['ray', data.cfRay],
      ]),
      hop('Zero Trust Tunnel (cloudflared)'),
      card('Homelab / local dev', [
        ['namespace', data.namespace],
        ['service', 'jaydanhoward (Foster port)'],
      ]),
      hop('kube-proxy → Pod'),
      card('This process', [
        ['pod', data.podName],
        ['node', data.nodeName],
        ['runtime', 'Rust · Foster · Axum'],
      ]),
    ].join('');
  }

  button?.addEventListener('click', load);
  await load();
}
