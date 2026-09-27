// Capybaras sidecar.
//
// For M3 this stands in for the Gateway: a Node process the shell must
// supervise correctly. It exists to prove three things:
//   1. the shell can spawn it from an app-local path,
//   2. a graceful stop actually reaches it (spike 001 showed a plain kill does
//      not fire handlers on Windows),
//   3. it dies when the shell dies, with no orphan.
//
// Protocol: newline-delimited JSON on stdin. Commands:
//   {"cmd":"shutdown"}  -> write a marker, exit cleanly. This is the graceful path.

import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.split('=')[1];
const stateDir = arg('state');
const started = Date.now();

function stamp() {
  return `+${String(Date.now() - started).padStart(6)}ms`;
}

let logFile = null;
function say(msg) {
  const line = `${stamp()}  ${msg}`;
  process.stdout.write(`${line}\n`);
  if (logFile) {
    try {
      appendFileSync(logFile, `${line}\n`);
    } catch {
      /* logging must never be the thing that kills the sidecar */
    }
  }
}

if (stateDir) {
  try {
    mkdirSync(stateDir, { recursive: true });
    logFile = join(stateDir, 'sidecar.log');
    writeFileSync(logFile, '');
  } catch (e) {
    process.stdout.write(`could not open state dir: ${e.message}\n`);
  }
}

say(`sidecar up pid=${process.pid} node=${process.version}`);
say(`  execPath=${process.execPath}`);
say(`  cwd=${process.cwd()}`);

const beats = setInterval(() => say('heartbeat'), 5000);

function shutdown(reason) {
  say(`GRACEFUL STOP RECEIVED (${reason}) -- handlers ran, exiting 0`);
  clearInterval(beats);
  // Exit on the next tick so the log write above flushes.
  setTimeout(() => process.exit(0), 50);
}

process.stdin.setEncoding('utf8');
let buf = '';
process.stdin.on('data', (chunk) => {
  buf += chunk;
  let nl;
  while ((nl = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    try {
      const msg = JSON.parse(line);
      if (msg.cmd === 'shutdown') shutdown('ipc');
      else say(`unknown command: ${msg.cmd}`);
    } catch {
      say(`unparseable input ignored: ${line}`);
    }
  }
});

process.stdin.on('end', () => shutdown('stdin closed'));
process.on('SIGTERM', () => shutdown('SIGTERM'));
process.on('SIGINT', () => shutdown('SIGINT'));
