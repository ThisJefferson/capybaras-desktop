// Capybaras interface behaviour.
//
// External, not inline: the app's CSP is `script-src 'self'`, which silently
// blocks inline scripts.
//
// The rule this file exists to honour: **a person who acts without reading
// cannot accidentally approve something destructive.** Enter is wired to the
// safe action everywhere in the card, including inside the confirmation field.

const tauri = window.__TAURI__ ?? {};
const invoke = tauri.invoke;
const listen = tauri.event?.listen;

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
} else {
  log('not running inside Tauri — the card cannot be driven from here');
}

/* ------------------------------------------------------------------ */
/* Test harness — no agent is wired up yet, so propose by hand        */
/* ------------------------------------------------------------------ */

let sequence = 0;
for (const button of document.querySelectorAll('.dev button[data-tool]')) {
  button.addEventListener('click', async () => {
    const id = `dev-${++sequence}`;
    const action = { tool: button.dataset.tool };
    if (button.dataset.count) action.affectedCount = Number(button.dataset.count);
    log(`propose ${id}: ${button.dataset.tool}`);
    try {
      await invoke('propose_action', { id, action });
    } catch (error) {
      log(`propose failed: ${error}`);
    }
  });
}

refreshStatus();
setInterval(refreshStatus, 5000);
