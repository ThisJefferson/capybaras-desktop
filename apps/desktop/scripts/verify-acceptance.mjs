// The acceptance test, through the REAL interface.
//
// WHY THIS EXISTS. `tests/replit-acceptance.test.ts` proves the *logic*, and
// `tests/protocol.rs::the_replit_incident_is_stopped_at_the_gate` proves the
// *plumbing*. Neither one shows the CARD: that it appears at all, says in plain
// words what it intends to do, keeps "Go ahead" disabled until the phrase is
// typed, and runs nothing until a person clicks. That is the milestone gate, and
// this is the only check that looks at it.
//
// WHAT IS REAL. The page is the real `index.html`, the real `app.js`, the real
// `app.css` and the real `tokens.css`. The gate is the **real sidecar**, spawned
// the way the shell spawns it and spoken to over protocol v1. A click in the page
// travels the product's own path: button -> `invoke('propose_action')` -> the
// gateway -> `action.propose` -> the classifier -> the card.
//
// THE ONE SUBSTITUTION, stated rather than hidden. `window.__TAURI__` is provided
// by `harness-bridge.js` instead of the Rust shell's webview, because this
// check runs without a desktop (see docs/DEBUGGING.md -- a window cannot be
// captured on this host). It is a transport shim and nothing more. No product
// decision is re-implemented here: the harness only moves bytes.
//
// Usage: npm run verify:acceptance   (builds the sidecar first)
//        node apps/desktop/scripts/verify-acceptance.mjs

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const web = join(repoRoot, 'apps', 'desktop', 'web');
const sidecarScript = join(repoRoot, 'apps', 'desktop', 'sidecar', 'dist', 'sidecar.mjs');

/* ------------------------------------------------------------------ */
/* Small helpers                                                       */
/* ------------------------------------------------------------------ */

const failures = [];
function check(label, ok, detail) {
  console.log(`${ok ? '  ok  ' : '  FAIL'}  ${label}${detail ? `  -- ${detail}` : ''}`);
  if (!ok) failures.push(label);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function chromePath() {
  const candidates = [
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    join(process.env.LOCALAPPDATA ?? '', 'Google', 'Chrome', 'Application', 'chrome.exe'),
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ];
  const found = candidates.find((p) => p && existsSync(p));
  if (!found) throw new Error(`Chrome not found. Tried: ${candidates.join(', ')}`);
  return found;
}

/* ------------------------------------------------------------------ */
/* The real sidecar                                                    */
/* ------------------------------------------------------------------ */

if (!existsSync(sidecarScript)) {
  console.error(`the sidecar bundle is missing: ${sidecarScript}\nRun: npm run build:sidecar`);
  process.exit(1);
}

const stateDir = mkdtempSync(join(tmpdir(), 'capybaras-acceptance-'));
const sidecar = spawn(process.execPath, [sidecarScript, `--state=${stateDir}`], {
  stdio: ['pipe', 'pipe', 'pipe'],
});

/** Every message the sidecar has sent, in order. */
const messages = [];
/** The subset that the shell turns into events, in the shell's own vocabulary. */
const events = [];
const liveEvents = [];

const EVENT_NAMES = {
  'approval.required': 'capybaras://approval-required',
  'approval.resolved': 'capybaras://approval-resolved',
  'action.proceeded': 'capybaras://action-proceeded',
  'action.dry_run': 'capybaras://action-dry-run',
  'agents.state': 'capybaras://agents',
  'grants.listed': 'capybaras://grants',
  error: 'capybaras://protocol-error',
  ready: 'capybaras://ready',
};

let lineBuffer = '';
sidecar.stdout.setEncoding('utf8');
sidecar.stdout.on('data', (chunk) => {
  lineBuffer += chunk;
  let index;
  while ((index = lineBuffer.indexOf('\n')) >= 0) {
    const line = lineBuffer.slice(0, index).trim();
    lineBuffer = lineBuffer.slice(index + 1);
    if (!line) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      continue; // a malformed line costs one message, never the stream
    }
    messages.push(message);
    const name = EVENT_NAMES[message.type];
    if (!name) continue;
    const record = { event: name, payload: message };
    events.push(record);
    liveEvents.push(record);
  }
});
sidecar.stderr.setEncoding('utf8');
sidecar.stderr.on('data', (text) => process.stderr.write(`[sidecar] ${text}`));

function sendToSidecar(message) {
  sidecar.stdin.write(`${JSON.stringify({ v: 1, ...message })}\n`);
}

/** The most recent whole-herd message the sidecar sent, or null. */
function latestAgents() {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    if (events[i].event === 'capybaras://agents') return events[i].payload;
  }
  return null;
}

function waitForEvent(name, predicate = () => true, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tick = () => {
      const found = events.find((e) => e.event === name && predicate(e.payload));
      if (found) return resolve(found);
      if (Date.now() > deadline) {
        return reject(new Error(`never saw ${name}; sidecar sent: ${events.map((e) => e.event).join(', ')}`));
      }
      setTimeout(tick, 50);
    };
    tick();
  });
}

/* ------------------------------------------------------------------ */
/* The harness server: the real frontend, the real sidecar behind it   */
/* ------------------------------------------------------------------ */

const STATIC = new Map([
  ['/tokens.css', join(web, 'tokens.css')],
  ['/app.css', join(web, 'app.css')],
  ['/app.js', join(web, 'app.js')],
  ['/harness-bridge.js', join(here, 'harness-bridge.js')],
]);

const ANCHOR = '<script type="module" src="app.js">';
const indexHtml = readFileSync(join(web, 'index.html'), 'utf8');
if (!indexHtml.includes(ANCHOR)) {
  throw new Error('could not find the app.js script tag in index.html -- has the page changed?');
}
// The bridge goes BEFORE app.js. app.js is a module and therefore deferred; a
// classic script ahead of it runs first, so the bridge exists by the time app.js
// reads it. This mirrors exactly what verify-ui.ps1 does, for the same reason.
const harnessHtml = indexHtml.replace(
  ANCHOR,
  `<script src="harness-bridge.js"></script>\n  ${ANCHOR}`,
);

function invoke(cmd, args) {
  switch (cmd) {
    case 'shell_status':
      return {
        shell_pid: process.pid,
        exe_dir: repoRoot,
        state_dir: stateDir,
        node_path: process.execPath,
        node_exists: true,
        sidecar_running: true,
        sidecar_healthy: true,
        sidecar_pid: sidecar.pid,
        job_assigned: true,
        job_error: null,
      };
    case 'connect_status':
      return { store_available: true, connected: false };
    case 'usage_status':
      return { calls: 0, total_tokens: 0, cost_display: '$0.00', credit: null };
    case 'list_grants':
      sendToSidecar({ type: 'grants.list' });
      return null;
    case 'revoke_grant':
      sendToSidecar({ type: 'grants.revoke', scope: { id: args.id } });
      return null;
    case 'propose_action':
      sendToSidecar({
        type: 'action.propose',
        id: args.id,
        action: args.action,
        ...(args.context ? { context: args.context } : {}),
      });
      return null;
    case 'answer_approval':
      sendToSidecar({ type: 'approval.answer', id: args.id, decision: args.decision });
      return null;
    case 'start_connect':
      return 41789;
    default:
      return null;
  }
}

function readBody(req) {
  return new Promise((resolve) => {
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => resolve(body));
  });
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://127.0.0.1');
  const json = (value) => {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify(value));
  };

  if (url.pathname === '/') {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    return res.end(harnessHtml);
  }

  if (STATIC.has(url.pathname)) {
    const file = STATIC.get(url.pathname);
    const type = url.pathname.endsWith('.css') ? 'text/css' : 'text/javascript';
    res.writeHead(200, { 'content-type': `${type}; charset=utf-8` });
    return res.end(readFileSync(file, 'utf8'));
  }

  if (url.pathname === '/invoke' && req.method === 'POST') {
    const { cmd, args } = JSON.parse(await readBody(req));
    try {
      return json({ result: invoke(cmd, args ?? {}) });
    } catch (error) {
      return json({ result: null, error: String(error) });
    }
  }

  if (url.pathname === '/events') {
    const since = Number(url.searchParams.get('since') ?? '0');
    const batch = liveEvents.slice(since);
    return json({ next: since + batch.length, events: batch });
  }

  res.writeHead(404);
  res.end('not found');
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;

/* ------------------------------------------------------------------ */
/* A minimal CDP client, so the page can be driven and asserted on     */
/* ------------------------------------------------------------------ */

function connectCdp(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map();
    let nextId = 1;
    const api = {
      send(method, params) {
        const id = nextId++;
        return new Promise((res, rej) => {
          pending.set(id, { res, rej });
          ws.send(JSON.stringify({ id, method, params: params ?? {} }));
        });
      },
      close() {
        try {
          ws.close();
        } catch {
          /* already closing */
        }
      },
    };
    ws.addEventListener('message', (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch {
        return;
      }
      const slot = message.id !== undefined && pending.get(message.id);
      if (!slot) return;
      pending.delete(message.id);
      if (message.error) slot.rej(new Error(message.error.message));
      else slot.res(message.result);
    });
    ws.addEventListener('open', () => resolve(api));
    ws.addEventListener('error', () => reject(new Error('could not open the CDP socket')));
  });
}

const profileDir = mkdtempSync(join(tmpdir(), 'capybaras-chrome-'));
const chrome = spawn(
  chromePath(),
  [
    '--headless=new',
    '--disable-gpu',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-timer-throttling',
    '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    '--remote-debugging-port=0',
    `--user-data-dir=${profileDir}`,
    'about:blank',
  ],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);

const devtoolsUrl = await new Promise((resolve, reject) => {
  let text = '';
  const timer = setTimeout(() => reject(new Error('Chrome did not report a DevTools endpoint')), 20000);
  chrome.stderr.setEncoding('utf8');
  chrome.stderr.on('data', (chunk) => {
    text += chunk;
    const match = /DevTools listening on (ws:\/\/\S+)/.exec(text);
    if (match) {
      clearTimeout(timer);
      resolve(match[1]);
    }
  });
  chrome.on('exit', () => reject(new Error('Chrome exited before it was ready')));
});

const port = new URL(devtoolsUrl).port;
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === 'page');
if (!page) throw new Error('no page target to drive');
const cdp = await connectCdp(page.webSocketDebuggerUrl);

async function evaluate(expression) {
  const result = await cdp.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails) {
    const detail = result.exceptionDetails.exception?.description ?? result.exceptionDetails.text;
    throw new Error(`the page threw: ${detail}`);
  }
  return result.result.value;
}

async function waitUntil(expression, ok, label, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = await evaluate(expression);
    if (ok(last)) return last;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label} (last value: ${JSON.stringify(last)})`);
}

const CARD = `JSON.stringify({
  headline: document.getElementById('card-headline').textContent,
  reasons: Array.from(document.querySelectorAll('#card-reasons li')).map((li) => li.textContent),
  warnShown: document.getElementById('card-warn').hidden === false,
  warn: document.getElementById('card-warn').textContent,
  phraseShown: document.getElementById('card-phrase').hidden === false,
  phrase: document.getElementById('card-phrase-text').textContent,
  goDisabled: document.getElementById('btn-go').disabled,
  hold: document.getElementById('btn-hold').textContent,
  rememberShown: document.getElementById('card-remember').hidden === false
})`;

async function readCard() {
  return JSON.parse(await evaluate(CARD));
}

/* ------------------------------------------------------------------ */
/* The run                                                             */
/* ------------------------------------------------------------------ */

console.log('=== the acceptance test, through the interface ===');

let exitCode = 0;
try {
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 900,
    height: 1200,
    deviceScaleFactor: 2,
    mobile: false,
  });
  await cdp.send('Page.navigate', { url: `${origin}/` });
  await waitUntil("document.readyState === 'complete'", (v) => v === true, 'the page to load');

  // ---- the herd renders before anything is asked of it (M4.6) ----
  const agents = await waitUntil(
    "document.querySelectorAll('[data-agent]').length",
    (v) => v === 6,
    'the herd to render',
  );
  check('the herd renders all six, and they are the product’s own', agents === 6, `found ${agents}`);

  // Nothing has been proposed by the page on its own.
  check(
    'nothing proceeds on load — no action is proposed by the interface itself',
    !events.some((e) => e.event === 'capybaras://action-proceeded'),
  );

  // ---- run the documented scenario: the Replit incident ----
  await evaluate("document.querySelector('[data-scenario=\"replit\"]').click(); true");
  await waitUntil("document.getElementById('card').hidden === false", (v) => v === true, 'the card');
  const card = await readCard();

  // 1. It says plainly what it intends to do, in words a person can read.
  check('the card states what it will do', /^Delete/.test(card.headline), card.headline);
  check('the headline leaks no tool identifier', !card.headline.includes('db.delete'), card.headline);

  // 2. It explains WHY — every reason, not just the first (D10).
  const reasons = card.reasons.join(' ');
  check('it lists the reasons, at least three', card.reasons.length >= 3, `${card.reasons.length}`);
  check('it states the target is off-limits', /off-limits/i.test(reasons));
  check('it states the blast radius', /1,?200/.test(reasons));
  check('it states it cannot be undone', /cannot be undone/i.test(reasons));

  // 3. It demands a typed confirmation, and refuses to be remembered.
  check('a typed confirmation is demanded', card.phraseShown, `phrase: "${card.phrase}"`);
  check('the hard gate is named as such', card.warnShown && /hard gate/i.test(card.warn));
  check('“always allow” is never offered for a hard gate', card.rememberShown === false);
  check('“Hold on” is the button, and it is the filled primary', /hold on/i.test(card.hold), card.hold);

  // 4. THE ASSERTION THAT MATTERS. It cannot proceed without a click.
  check('“Go ahead” is DISABLED until the phrase is typed', card.goDisabled === true);

  // 5. A real state change in the sidecar produced the correct visible state (M4.6).
  //
  // The owner is taken from the sidecar's OWN report rather than hardcoded: a
  // check that names the wrong agent is a check that proves nothing, and the
  // registry -- not the harness -- decides who owns a tool.
  const needing = (latestAgents()?.agents ?? []).filter((a) => a.state === 'needs-you');
  check(
    'exactly one agent raises the sign — the loudest state is never a badge',
    needing.length === 1,
    `${needing.length} agent(s) need you`,
  );
  const owner = needing[0] ?? { id: 'zeca', label: 'Zeca' };
  const ownerDomState = await evaluate(
    `document.querySelector('[data-agent="${owner.id}"]').dataset.state`,
  );
  check(
    'the state the sidecar reported is the state the interface shows',
    ownerDomState === 'needs-you',
    `${owner.label} owns this tool and shows "${ownerDomState}"`,
  );

  // ---- nothing runs while the card waits ----
  await sleep(2500);
  check(
    'nothing proceeds while the card waits (2.5s) — the wait is real, not a timer',
    !events.some((e) => e.event === 'capybaras://action-proceeded'),
  );

  // The halt, with "Go ahead" still disabled: this is the milestone-gate image.
  const haltShot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const haltPath = join(here, 'acceptance-halt.png');
  writeFileSync(haltPath, Buffer.from(haltShot.data, 'base64'));
  console.log(`        screenshot -> ${haltPath}`);

  // ---- the phrase unlocks it, and only the phrase ----
  await evaluate(`(() => {
    const input = document.getElementById('card-phrase-input');
    input.value = document.getElementById('card-phrase-text').textContent;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  })()`);
  const unlocked = await evaluate("document.getElementById('btn-go').disabled");
  check('“Go ahead” becomes available once the phrase is typed', unlocked === false);

  // ---- a click proceeds, down the product's own path ----
  await evaluate("document.getElementById('btn-go').click(); true");
  const proceeded = await waitForEvent('capybaras://action-proceeded');
  check('an explicit click does proceed, over the real protocol', proceeded.payload.type === 'action.proceeded');
  const settled = await waitUntil(
    `document.querySelector('[data-agent="${owner.id}"]').dataset.state`,
    (v) => v === 'listening',
    'the sign to come down',
  );
  check(`the sign comes down once the decision is made (${owner.label} -> listening)`, settled === 'listening');

  // ---- the reflex is the safe action: Enter holds on ----
  const before = events.length;
  await evaluate("document.querySelector('[data-scenario=\"replit\"]').click(); true");
  await waitUntil("document.getElementById('card').hidden === false", (v) => v === true, 'the card again');
  await evaluate("document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); true");
  const denied = await waitForEvent('capybaras://approval-resolved', (p) => p.outcome === 'denied');
  check('Enter holds on rather than approving — the reflex cannot proceed', denied.payload.outcome === 'denied');
  const after = events.slice(before);
  check(
    'a held-on action never proceeds',
    !after.some((e) => e.event === 'capybaras://action-proceeded'),
  );
} catch (error) {
  console.error(`\nERROR: ${error.message}`);
  failures.push(error.message);
} finally {
  cdp.close();
  try {
    chrome.kill();
  } catch {
    /* already gone */
  }
  try {
    sidecar.stdin.write('{"cmd":"shutdown"}\n');
  } catch {
    /* already gone */
  }
  await sleep(300);
  try {
    sidecar.kill();
  } catch {
    /* already gone */
  }
  server.close();
  try {
    rmSync(stateDir, { recursive: true, force: true });
    rmSync(profileDir, { recursive: true, force: true });
  } catch {
    /* windows sometimes holds the profile briefly */
  }
}

console.log('');
if (failures.length > 0) {
  console.log(`FAILED — ${failures.length} check(s):`);
  for (const f of failures) console.log(`  - ${f}`);
  exitCode = 1;
} else {
  console.log('PASSED — the documented scenario halts, states plainly what it intends,');
  console.log('         and cannot proceed without a click.');
}
process.exit(exitCode);
