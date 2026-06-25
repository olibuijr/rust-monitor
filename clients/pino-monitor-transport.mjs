// Dependency-free pino destination that ships logs to rust-monitor's /api/ingest.
//
// Usage:
//   import pino from "pino";
//   import { createMonitorStream } from "./pino-monitor-transport.mjs";
//
//   const monitor = createMonitorStream({
//     url: process.env.MONITOR_INGEST_URL,      // https://monitor.olibuijr.com/api/ingest
//     token: process.env.MONITOR_INGEST_TOKEN,  // shared bearer token
//     source: "akurai-mail",                    // app name shown in the monitor
//   });
//   export const logger = pino({ level: "info" }, monitor);
//
// Notes:
// - Runs in-process (no worker thread, no extra deps). Batches lines and POSTs
//   them; network failures are swallowed so logging never crashes the app.
// - Requires global fetch (Node >= 18 or Bun).

const LEVELS = { 10: "TRACE", 20: "DEBUG", 30: "INFO", 40: "WARN", 50: "ERROR", 60: "FATAL" };

export function createMonitorStream({
  url,
  token,
  source,
  batchSize = 50,
  flushMs = 2000,
  alsoStdout = true,
}) {
  if (!url || !token || !source) {
    throw new Error("createMonitorStream requires { url, token, source }");
  }

  let buf = [];
  let timer = null;

  async function flush() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (buf.length === 0) return;
    const logs = buf;
    buf = [];
    try {
      await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
        body: JSON.stringify({ logs }),
      });
    } catch {
      // Drop on failure — never let log shipping break the app.
    }
  }

  function schedule() {
    if (!timer) timer = setTimeout(flush, flushMs);
  }

  return {
    write(chunk) {
      if (alsoStdout) process.stdout.write(chunk);

      let line = chunk;
      let ts;
      try {
        const o = JSON.parse(chunk);
        ts = o.time ? Math.floor(o.time / 1000) : undefined;
        const lvl = LEVELS[o.level] || String(o.level ?? "");
        const { level, time, pid, hostname, msg, ...rest } = o;
        const extra = Object.keys(rest).length ? " " + JSON.stringify(rest) : "";
        line = `${lvl} ${msg ?? ""}${extra}`.trim();
      } catch {
        line = String(chunk).trim();
      }

      buf.push({ source, line, ts });
      if (buf.length >= batchSize) flush();
      else schedule();
    },
  };
}
