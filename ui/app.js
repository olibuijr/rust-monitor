const $ = (s) => document.querySelector(s);
const app = $('#app');

async function api(path) {
  const res = await fetch(path);
  return res.json();
}

function ts(epoch) {
  return new Date(epoch * 1000).toLocaleString();
}

function severity(name, value) {
  if (name.includes('used_pct') || name === 'cpu.usage' || name === 'mem.used_pct') {
    if (value >= 90) return 'crit';
    if (value >= 75) return 'warn';
    return 'ok';
  }
  return 'ok';
}

function formatValue(name, value) {
  if (name.includes('_kb')) return (value / 1024 / 1024).toFixed(1) + ' GB';
  if (name.includes('_gb')) return value.toFixed(1) + ' GB';
  if (name.includes('_pct') || name === 'cpu.usage') return value.toFixed(1) + '%';
  if (name.includes('bytes')) return formatBytes(value);
  if (name === 'uptime.seconds') return formatDuration(value);
  if (name.startsWith('load.')) return value.toFixed(2);
  return value.toFixed(1);
}

function formatBytes(b) {
  if (b < 1024) return b + ' B';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
  if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
  return (b / 1073741824).toFixed(1) + ' GB';
}

function formatDuration(secs) {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  return d > 0 ? `${d}d ${h}h` : `${h}h`;
}

function metricLabel(name) {
  const labels = {
    'cpu.usage': 'CPU Usage',
    'mem.used_pct': 'Memory',
    'mem.total_kb': 'Total RAM',
    'mem.used_kb': 'Used RAM',
    'mem.available_kb': 'Available RAM',
    'load.1m': 'Load 1m',
    'load.5m': 'Load 5m',
    'load.15m': 'Load 15m',
    'uptime.seconds': 'Uptime',
  };
  if (labels[name]) return labels[name];
  if (name.startsWith('disk.')) return name.replace('disk.', 'Disk ').replace('.used_pct', ' Usage').replace('.total_gb', ' Total').replace('.used_gb', ' Used');
  if (name.startsWith('net.')) return name.replace('net.', 'Net ').replace('.rx_bytes', ' RX').replace('.tx_bytes', ' TX');
  return name;
}

// Priority order for dashboard cards
const cardOrder = ['cpu.usage', 'mem.used_pct', 'load.1m', 'uptime.seconds'];

async function renderDashboard() {
  const data = await api('/api/status');
  const alerts = await api('/api/alerts?hours=24');

  const metrics = data.metrics || [];
  // Sort: priority cards first, then alphabetical
  const sorted = [...metrics].sort((a, b) => {
    const ai = cardOrder.indexOf(a.name);
    const bi = cardOrder.indexOf(b.name);
    if (ai >= 0 && bi >= 0) return ai - bi;
    if (ai >= 0) return -1;
    if (bi >= 0) return 1;
    return a.name.localeCompare(b.name);
  });

  let html = '<div class="refresh">Auto-refreshes every 60s</div>';
  html += '<h2>Current Status</h2><div class="grid">';
  for (const m of sorted) {
    const sev = severity(m.name, m.value);
    html += `<div class="card"><div class="label">${metricLabel(m.name)}</div><div class="value ${sev}">${formatValue(m.name, m.value)}</div></div>`;
  }
  html += '</div>';

  // Active alerts
  const active = (alerts.alerts || []).filter(a => !a.resolved_at);
  if (active.length > 0) {
    html += '<h2>Active Alerts</h2><table><tr><th>Rule</th><th>Metric</th><th>Value</th><th>Since</th></tr>';
    for (const a of active) {
      html += `<tr class="alert-active"><td>${a.rule_name}</td><td>${a.metric_name}</td><td>${a.value.toFixed(1)}</td><td>${ts(a.triggered_at)}</td></tr>`;
    }
    html += '</table>';
  }

  if (metrics.length === 0) {
    html = '<div class="empty">No metrics yet. Waiting for first collection cycle...</div>';
  }

  app.innerHTML = html;
}

async function renderLogs() {
  const data = await api('/api/logs?hours=1');
  const logs = data.logs || [];

  let html = '<h2>Recent Logs (1 hour)</h2>';
  if (logs.length === 0) {
    html += '<div class="empty">No log entries</div>';
  } else {
    html += '<table><tr><th>Time</th><th>Source</th><th>Line</th></tr>';
    for (const l of logs) {
      const src = l.source.split('/').pop();
      html += `<tr><td style="white-space:nowrap">${ts(l.ts)}</td><td>${src}</td><td>${escapeHtml(l.line)}</td></tr>`;
    }
    html += '</table>';
  }

  app.innerHTML = html;
}

async function renderAlerts() {
  const data = await api('/api/alerts?hours=72');
  const alerts = data.alerts || [];

  let html = `<h2>Alerts (72 hours) &mdash; ${data.active || 0} active</h2>`;
  if (alerts.length === 0) {
    html += '<div class="empty">No alerts</div>';
  } else {
    html += '<table><tr><th>Status</th><th>Rule</th><th>Metric</th><th>Value</th><th>Triggered</th><th>Resolved</th></tr>';
    for (const a of alerts) {
      const cls = a.resolved_at ? 'alert-resolved' : 'alert-active';
      const status = a.resolved_at ? 'Resolved' : 'Active';
      html += `<tr class="${cls}"><td>${status}</td><td>${a.rule_name}</td><td>${a.metric_name}</td><td>${a.value.toFixed(1)}</td><td>${ts(a.triggered_at)}</td><td>${a.resolved_at ? ts(a.resolved_at) : '-'}</td></tr>`;
    }
    html += '</table>';
  }

  app.innerHTML = html;
}

function escapeHtml(s) {
  const div = document.createElement('div');
  div.textContent = s;
  return div.innerHTML;
}

function route() {
  const hash = location.hash || '#/';
  document.querySelectorAll('nav a').forEach(a => {
    a.classList.toggle('active', a.getAttribute('href') === hash);
  });

  if (hash.startsWith('#/logs')) renderLogs();
  else if (hash.startsWith('#/alerts')) renderAlerts();
  else renderDashboard();
}

window.addEventListener('hashchange', route);
route();

// Auto-refresh every 60s
setInterval(route, 60000);
