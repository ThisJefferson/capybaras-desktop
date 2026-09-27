// Capybaras interface behaviour.
//
// External, not inline: the app's CSP is `script-src 'self'`, which silently
// blocks inline scripts.
//
// The rule this file exists to honour: **a person who acts without reading
// cannot accidentally approve something destructive.** Enter is wired to the
// safe action everywhere in the card, including inside the confirmation field.

/* ------------------------------------------------------------------ */
/* The Tauri bridge                                                    */
/* ------------------------------------------------------------------ */

// WHERE THE BRIDGE ACTUALLY LIVES, and why this is written defensively.
//
// Tauri v1 exposed `invoke` at the top level. **Tauri v2 moved it under `core`**
// -- `window.__TAURI__.core.invoke` -- while `event.listen` stayed top level.
// (Verified against tauri 2.12.0's own injected bundle, which ends
// `...e.window=he,e}({});window.__TAURI__=__TAURI_IIFE__;`.)
//
// The first version of this file used the v1 shape. The app then launched looking
// perfectly healthy while EVERY button failed with "invoke is not a function" in
// a log nobody was reading.
//
// The harness did not catch it because the harness MOCKED the bridge -- and the
// mock was written with the same wrong assumption. That is the lesson worth more
// than the fix: **a mock that invents the shape of an external API validates the
// author's belief about that API, not the API.** The mock now mirrors the real v2
// shape, so a regression here fails the smoke test instead of shipping.
const tauri = window.__TAURI__ ?? {};
const invoke = tauri.core?.invoke ?? tauri.invoke;
const listen = tauri.event?.listen ?? tauri.core?.event?.listen;

// Fail LOUDLY. Until now a missing bridge produced one TypeError per click in a
// log, which reads as "the buttons are broken" rather than "nothing in this
// window is connected to anything". A silent failure that looks like a broken
// button is the worst possible presentation of a total failure.
if (typeof invoke !== 'function') {
  const banner = document.createElement('div');
  banner.className = 'banner banner--error';
  banner.textContent =
    'Capybaras is not connected to its engine, so nothing in this window will work. ' +
    'Close this window and start the app again with run.cmd.';
  document.body.prepend(banner);
  log('FATAL: no Tauri bridge — window.__TAURI__ has no callable invoke. Nothing will work.');
}

const $ = (id) => document.getElementById(id);
/** The approval currently awaiting an answer, if any. */
let current = null;

function log(line) {
  const el = $('log');
  const time = new Date().toLocaleTimeString();
  el.textContent = `[${time}] ${line}\n${el.textContent}`;
}

/* ------------------------------------------------------------------ */
/* Status                                                             */
/* ------------------------------------------------------------------ */

function row(label, value, className) {
  const dt = document.createElement('dt');
  dt.textContent = label;
  const dd = document.createElement('dd');
  dd.textContent = value;
  if (className) dd.className = className;
  return [dt, dd];
}

async function refreshStatus() {
  if (!invoke) return;
  const dl = $('status');
  try {
    const s = await invoke('shell_status');
    dl.textContent = '';
    dl.append(...row('shell pid', String(s.shell_pid)));
    dl.append(...row('state dir', s.state_dir));
    dl.append(
      ...row(
        'sidecar',
        s.sidecar_running ? `running (pid ${s.sidecar_pid})` : 'not running',
        s.sidecar_running ? 'ok' : 'bad',
      ),
    );
    dl.append(
      ...row(
        'job object',
        s.job_assigned ? 'anchored — no orphans' : 'NOT assigned',
        s.job_assigned ? 'ok' : 'bad',
      ),
    );
  } catch (error) {
    dl.textContent = '';
    dl.append(...row('error', String(error), 'bad'));
  }
}

/* ------------------------------------------------------------------ */
/* Connecting a model                                                  */
/* ------------------------------------------------------------------ */

/**
 * Whether a model is connected, and whether this app can hold a key at all.
 *
 * If the OS credential store is unavailable, the button is DISABLED and the reason
 * is shown, rather than being offered and then failing. That is the honest failure
 * direction: Capybaras will not fall back to keeping a key in a file, because a key
 * in a file is one the agent can read and then use outside the gate (T12).
 *
 * The key itself never reaches this function. Only whether one exists.
 */
async function refreshConnect() {
  if (!invoke) return;
  const state = $('connect-state');
  const button = $('btn-connect');
  if (!state || !button) return;

  try {
    const status = await invoke('connect_status');
    if (!status.store_available) {
      state.textContent =
        'Capybaras cannot reach the Windows credential store, so it cannot keep a key. ' +
        'It will not store one anywhere else.';
      state.className = 'hint bad';
      button.hidden = true;
      return;
    }
    state.textContent = status.connected ? 'Connected.' : 'Not connected yet.';
    state.className = status.connected ? 'hint ok' : 'hint';
    button.hidden = status.connected;
    button.disabled = status.connected;
  } catch (error) {
    state.textContent = `Could not check the connection: ${error}`;
    state.className = 'hint bad';
  }
}

const connectButton = $('btn-connect');
if (connectButton) {
  connectButton.addEventListener('click', async () => {
    // Disabled immediately: a second click would start a second sign-in, and the
    // user would have no way to tell which browser tab belonged to which.
    connectButton.disabled = true;
    try {
      const port = await invoke('start_connect');
      log(`sign-in started — finish it in your browser (port ${port})`);
    } catch (error) {
      log(`could not start the sign-in: ${error}`);
      connectButton.disabled = false;
    }
  });
}

/* ------------------------------------------------------------------ */
/* The card                                                           */
/* ------------------------------------------------------------------ */

function showCard(id, request) {
  current = { id, request };
  $('card').hidden = false;

  $('card-headline').textContent = request.headline;

  // Every reason, not just the first. A card that hides reasons is a card that
  // teaches people to click through it.
  const reasons = $('card-reasons');
  reasons.textContent = '';
  const list = request.reasons?.length ? request.reasons : ['This cannot be undone.'];
  for (const reason of list) {
    const li = document.createElement('li');
    li.textContent = reason;
    reasons.append(li);
  }

  const warn = $('card-warn');
  if (request.tier === 'hard_gate') {
    warn.hidden = false;
    warn.textContent =
      'This is a hard gate. Capybaras will not remember it, and no number of past approvals will let it through on its own.';
  } else {
    warn.hidden = true;
  }

  const goButton = $('btn-go');
  const phraseBlock = $('card-phrase');
  if (request.requiresTypedConfirmation && request.confirmationPhrase) {
    phraseBlock.hidden = false;
    $('card-phrase-text').textContent = request.confirmationPhrase;
    const input = $('card-phrase-input');
    input.value = '';
    goButton.disabled = true;
    input.oninput = () => {
      goButton.disabled = input.value.trim() !== request.confirmationPhrase;
    };
  } else {
    phraseBlock.hidden = true;
    goButton.disabled = false;
  }

  // The remember affordance is only offered when the classifier says it may be.
  $('card-remember').hidden = !request.canRemember;
  $('card-remember-box').checked = false;

  log(`approval required: ${request.headline} [${request.tier}]`);
}

function hideCard() {
  $('card').hidden = true;
  current = null;
}

async function answer(decision) {
  if (!current) return;
  const { id } = current;
  log(`answering ${id}: ${decision}`);
  hideCard();
  try {
    await invoke('answer_approval', { id, decision });
  } catch (error) {
    log(`answer failed: ${error}`);
  }
}

$('btn-hold').addEventListener('click', () => answer('deny'));
$('btn-go').addEventListener('click', () =>
  answer($('card-remember-box').checked ? 'remember' : 'allow'),
);

// Enter is the safe action, everywhere in the card — including inside the
// confirmation field. The reflex cannot approve; it can only stop.
document.addEventListener('keydown', (event) => {
  if (event.key !== 'Enter' || $('card').hidden) return;
  event.preventDefault();
  answer('deny');
});

/* ------------------------------------------------------------------ */
/* The herd                                                            */
/* ------------------------------------------------------------------ */

// The character art is a PLACEHOLDER. The calçadão wave mark stands in until
// the commissioned model sheet exists (one character, four poses, one sign,
// D18). Swapping the art must not require touching this layout.
const WAVE_PATHS = [
  'M 60,152 C 95,130 123,130 158,152 C 193,174 221,174 256,152 C 291,130 319,130 354,152 C 389,174 417,174 452,152',
  'M 60,256 C 95,278 123,278 158,256 C 193,234 221,234 256,256 C 291,278 319,278 354,256 C 389,234 417,234 452,256',
  'M 60,360 C 95,338 123,338 158,360 C 193,382 221,382 256,360 C 291,338 319,338 354,360 C 389,382 417,382 452,360',
];

const STATE_LABEL = {
  dozing: 'dozing',
  listening: 'listening',
  working: 'working',
  'needs-you': 'needs you',
};

const SVG_NS = 'http://www.w3.org/2000/svg';

function buildAgent(agent) {
  const el = document.createElement('div');
  el.className = `agent agent--${agent.accent}`;
  el.dataset.agent = agent.id;
  el.title = `${agent.label} — ${agent.job}`;

  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 512 512');
  svg.setAttribute('class', 'agent__mark');
  svg.setAttribute('aria-hidden', 'true');
  for (const d of WAVE_PATHS) {
    const path = document.createElementNS(SVG_NS, 'path');
    path.setAttribute('class', 'wave');
    path.setAttribute('d', d);
    path.setAttribute('fill', 'none');
    path.setAttribute('stroke-width', '34');
    path.setAttribute('stroke-linecap', 'round');
    path.setAttribute('stroke-linejoin', 'round');
    svg.append(path);
  }

  const name = document.createElement('span');
  name.className = 'agent__name';
  name.textContent = agent.label;

  const state = document.createElement('span');
  state.className = 'agent__state';

  el.append(svg, name, state);
  return el;
}

function renderHerd(agents) {
  const host = $('herd');
  if (!host || !Array.isArray(agents)) return;

  // Build the row once, then update in place. Rebuilding would restart every
  // animation on each message, which would look twitchy rather than calm.
  if (host.childElementCount !== agents.length) {
    host.textContent = '';
    for (const agent of agents) host.append(buildAgent(agent));
  }

  for (const agent of agents) {
    const el = host.querySelector(`[data-agent="${agent.id}"]`);
    if (!el) continue;
    const label = STATE_LABEL[agent.state] ?? agent.state;
    el.dataset.state = agent.state;
    el.querySelector('.agent__state').textContent = label;
    el.setAttribute('aria-label', `${agent.label}, ${agent.job}: ${label}`);
  }
}

/* ------------------------------------------------------------------ */
/* Events pushed from the shell                                       */
/* ------------------------------------------------------------------ */

if (listen) {
  listen('capybaras://approval-required', (e) => showCard(e.payload.id, e.payload.request));
  listen('capybaras://approval-resolved', (e) => log(`resolved: ${e.payload.id} → ${e.payload.outcome}`));
  listen('capybaras://action-proceeded', (e) =>
    log(`proceeded: ${e.payload.id} (${e.payload.tier}) — ${e.payload.because ?? ''}`),
  );
  listen('capybaras://action-dry-run', (e) => log(`dry run: ${e.payload.headline}`));
  listen('capybaras://agents', (e) => renderHerd(e.payload.agents));
  listen('capybaras://protocol-error', (e) => log(`refused: ${e.payload.message}`));
  listen('capybaras://ready', (e) => log(`sidecar ready (protocol ${e.payload.protocol})`));
  listen('capybaras://connect-waiting', (e) =>
    log(`waiting for your browser to come back (port ${e.payload.port})`),
  );
  listen('capybaras://connect-connected', (e) => {
    log(`connected — a key is stored as "${e.payload.credentialName}" in Windows`);
    refreshConnect();
  });
  listen('capybaras://connect-failed', (e) => {
    log(`sign-in failed (${e.payload.reason}): ${e.payload.detail}`);
    refreshConnect();
  });
} else {
  log('not running inside Tauri — the card cannot be driven from here');
}

/* ------------------------------------------------------------------ */
/* Test harness — no agent is wired up yet, so propose by hand        */
/* ------------------------------------------------------------------ */

/* ------------------------------------------------------------------ */
/* Acceptance test harness                                            */
/* ------------------------------------------------------------------ */

let sequence = 0;

/**
 * The incident this product exists to stop.
 *
 * 23 July 2025: an agent with delete rights, repeatedly instructed not to
 * touch production, acting during an explicitly declared code and action
 * freeze, deleted a production database and then misreported what it had done.
 *
 * The claim under test is that this halts, says plainly what it intends to do,
 * and cannot proceed without a human.
 */
const REPLIT_SCENARIO = {
  tool: 'db.delete',
  args: { database: 'production', statement: 'DELETE FROM customers' },
  affectedCount: 1200,
  reversible: false,
  protectedTarget: true,
  // The *motivation* was legitimate; the action was not. This is what makes the
  // incident interesting: nobody was attacking, and it still nearly happened.
  taint: 'trusted',
};

async function propose(label, action, context) {
  const id = `dev-${++sequence}`;
  log(`propose ${id}: ${label}`);
  try {
    await invoke('propose_action', { id, action, context });
  } catch (error) {
    log(`propose failed: ${error}`);
  }
}

const scenarioButton = document.querySelector('[data-scenario="replit"]');
if (scenarioButton) {
  scenarioButton.addEventListener('click', () => {
    log('— running the acceptance test: an agent deleting production during a freeze —');
    propose('the production database, mid-freeze', REPLIT_SCENARIO, {
      targetLabel: 'the production database',
    });
  });
}

for (const button of document.querySelectorAll('.dev button[data-tool]')) {
  button.addEventListener('click', () => {
    const action = { tool: button.dataset.tool };
    if (button.dataset.count) action.affectedCount = Number(button.dataset.count);
    if (button.dataset.protected) action.protectedTarget = button.dataset.protected === 'true';
    const context = button.dataset.target ? { targetLabel: button.dataset.target } : undefined;
    propose(button.dataset.tool, action, context);
  });
}

refreshStatus();
setInterval(refreshStatus, 5000);
refreshConnect();
