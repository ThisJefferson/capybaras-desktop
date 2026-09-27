// SPIKE CODE — throwaway.
//
// The supervisor under test. In crash modes it deliberately gives the sidecar
// NO stdout pipe (stdio 'ignore') so we isolate the real question: does the
// sidecar survive the death of its parent?

import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const mode = process.argv[2] ?? 'clean';
const runDir = process.argv[3] ?? here;

const watchParent = mode === 'crash-watch';
const isCrash = mode.startsWith('crash');

const pidFile = join(runDir, `.sidecar-${mode}.pid`);
const logFile = join(runDir, `.sidecar-${mode}.log`);

const child = spawn(
  process.execPath,
  [
    join(here, 'sidecar.mjs'),
    `--parent=${process.pid}`,
    `--pidfile=${pidFile}`,
    `--logfile=${logFile}`,
  ].concat(watchParent ? ['--watch-parent'] : []),
  // No pipes to the parent at all: the pipe itself must not be able to kill it.
  { stdio: 'ignore', detached: false },
);

process.stdout.write(`supervisor pid=${process.pid} sidecar=${child.pid} mode=${mode} (stdio=ignore)\n`);

if (isCrash) {
  process.stdout.write('supervisor: idle, awaiting forcible termination\n');
  setInterval(() => {}, 1000);
} else {
  setTimeout(() => {
    process.stdout.write('supervisor: forward-killing sidecar\n');
    child.kill();
    setTimeout(() => process.exit(0), 300);
  }, 1600);
}
