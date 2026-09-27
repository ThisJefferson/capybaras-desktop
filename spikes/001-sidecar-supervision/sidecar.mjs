// SPIKE CODE — throwaway.
//
// Minimal sidecar. Writes heartbeats either to stdout or to a file, so the
// harness can separate two confounded effects: dying because the parent died,
// versus dying because the stdout pipe to the parent broke.

import { appendFileSync, writeFileSync } from 'node:fs';

const args = process.argv.slice(2);
const get = (name) => args.find((a) => a.startsWith(`--${name}=`))?.split('=')[1];

const parentPid = Number(get('parent') ?? 0);
const pidFile = get('pidfile');
const logFile = get('logfile');
const watchParent = args.includes('--watch-parent');

if (pidFile) writeFileSync(pidFile, String(process.pid));

const started = Date.now();
function say(msg) {
  const line = `+${String(Date.now() - started).padStart(6)}ms  ${msg}\n`;
  if (logFile) {
    try { appendFileSync(logFile, line); } catch { /* nothing we can do */ }
  } else {
    process.stdout.write(line);
  }
}

say(`sidecar up pid=${process.pid} watchParent=${watchParent} parent=${parentPid} logfile=${!!logFile}`);

let beats = 0;
const heartbeat = setInterval(() => {
  beats += 1;
  say(`beat ${beats} (parent alive: ${isParentAlive()})`);
}, 400);

function isParentAlive() {
  if (!parentPid) return false;
  try {
    process.kill(parentPid, 0);
    return true;
  } catch {
    return false;
  }
}

if (watchParent && parentPid > 0) {
  const watcher = setInterval(() => {
    if (!isParentAlive()) {
      say('PARENT GONE -> exiting by design');
      clearInterval(watcher);
      clearInterval(heartbeat);
      process.exit(0);
    }
  }, 200);
}
