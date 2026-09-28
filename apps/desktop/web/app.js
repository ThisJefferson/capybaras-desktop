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
  // A total failure is announced, not merely drawn: a screen reader should hear
  // this without hunting for it.
  banner.setAttribute('role', 'alert');
  banner.textContent =
    'Capybaras is not connected to its engine, so nothing in this window will work. ' +
    'Close this window and start the app again with run.cmd.';
  // Inside the shell rather than prepended to body: the header is sticky, and a
  // banner above it would scroll away behind it -- the opposite of unmissable.
  const shell = document.querySelector('.shell');
  if (shell) shell.prepend(banner);
  else document.body.prepend(banner);
  log('FATAL: no Tauri bridge — window.__TAURI__ has no callable invoke. Nothing will work.');
}

const $ = (id) => document.getElementById(id);
/** The approval currently awaiting an answer, if any. */
let current = null;

function log(line) {
  const el = $('log');
  if (!el) return;
  const time = new Date().toLocaleTimeString();
  el.textContent = `[${time}] ${line}\n${el.textContent}`;
  // The log had no empty state, so a quiet start looked like a broken panel.
  const empty = $('log-empty');
  if (empty) empty.hidden = true;
}

/* ------------------------------------------------------------------ */
/* Status                                                             */
/* ------------------------------------------------------------------ */

function row(label, value, className, chipKind) {
  const dt = document.createElement('dt');
  dt.textContent = label;
  const dd = document.createElement('dd');
  if (chipKind) {
    // A chip carries colour AND a word AND a shape, so the state survives being
    // read in greyscale or by a screen reader (brief section 7).
    const chip = document.createElement('span');
    chip.className = `chip chip--${chipKind}`;
    chip.textContent = value;
    dd.append(chip);
  } else {
    dd.textContent = value;
  }
  if (className) dd.classList.add(className);
  return [dt, dd];
}

/** The last status the shell reported, so an unchanged read leaves the DOM alone. */
let lastStatusKey = null;

async function refreshStatus() {
  if (!invoke) return;
  const dl = $('status');
  if (!dl) return;
  try {
    const s = await invoke('shell_status');
    // This runs every five seconds. Rebuilding four rows produces no change and
    // can reflow the monospace path text, so compare first and render only when
    // something is actually different.
    const key = [s.shell_pid, s.state_dir, s.sidecar_running, s.sidecar_pid, s.job_assigned].join('\u0000');
    if (key === lastStatusKey) return;
    lastStatusKey = key;
    dl.textContent = '';
    dl.setAttribute('aria-busy', 'false');
    dl.append(...row('shell pid', String(s.shell_pid)));
    dl.append(...row('state dir', s.state_dir));
    dl.append(
      ...row(
        'sidecar',
        s.sidecar_running ? `running (pid ${s.sidecar_pid})` : 'not running',
        null,
        s.sidecar_running ? 'success' : 'danger',
      ),
    );
    dl.append(
      ...row(
        'job object',
        s.job_assigned ? 'anchored — no orphans' : 'not assigned',
        null,
        s.job_assigned ? 'success' : 'danger',
      ),
    );
  } catch (error) {
    // A failed read must not be swallowed by the change check: clear it, so the
    // next success renders even if the figures happen to match the last good ones.
    lastStatusKey = null;
    dl.textContent = '';
    dl.setAttribute('aria-busy', 'false');
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
  const intro = $('before-connect');
  if (!state || !button) return;
  // Any settled answer clears the busy spinner; the button's own disabled and
  // hidden state then says whether there is anything left to do.
  button.removeAttribute('aria-busy');

  try {
    const status = await invoke('connect_status');
    if (!status.store_available) {
      state.textContent =
        'Capybaras cannot reach the Windows credential store, so it cannot keep a key. ' +
        'It will not store one anywhere else.';
      state.className = 'hint bad';
      button.hidden = true;
      if (intro) intro.hidden = true;
      return;
    }
    state.textContent = status.connected ? 'Connected.' : 'Not connected yet.';
    state.className = status.connected ? 'hint ok' : 'hint';
    button.hidden = status.connected;
    button.disabled = status.connected;
    // The first-run explanation is offered while there is still something to
    // decide, and withdrawn once a key is connected (M5 section 9).
    if (intro) intro.hidden = status.connected;

    // A user who connected in an earlier session should not have to know that a
    // "Load models" button exists before they can get an answer.
    if (status.connected && !modelsLoaded) loadModels();
  } catch (error) {
    state.textContent = `Could not check the connection: ${error}`;
    state.className = 'hint bad';
  }
}

/**
 * Say something on the connection panel.
 *
 * Exists because the connection state is reported from two directions -- the
 * `connect_status` read and the sign-in events -- and only one of them was ever
 * reaching the screen. The panel is the ONLY place a non-technical person looks;
 * the log is a developer's, and a message that appears only there is a message
 * that was not delivered.
 */
function setConnectState(text, kind) {
  const state = $('connect-state');
  if (!state) return;
  state.textContent = text;
  state.className = kind ? `hint ${kind}` : 'hint';
}

const connectButton = $('btn-connect');
if (connectButton) {
  connectButton.addEventListener('click', async () => {
    // Disabled immediately: a second click would start a second sign-in, and the
    // user would have no way to tell which browser tab belonged to which.
    connectButton.disabled = true;
    connectButton.setAttribute('aria-busy', 'true');
    try {
      const port = await invoke('start_connect');
      log(`sign-in started — finish it in your browser (port ${port})`);
    } catch (error) {
      log(`could not start the sign-in: ${error}`);
      connectButton.disabled = false;
      connectButton.removeAttribute('aria-busy');
    }
  });
}

/* ------------------------------------------------------------------ */
/* The card                                                           */
/* ------------------------------------------------------------------ */

function showCard(id, request) {
  current = { id, request };
  $('card').hidden = false;
  // The page behind the card must not scroll while it is up.
  document.body.classList.add('is-locked');

  // Focus goes INTO the dialog, and comes back when it closes. Without this the
  // single most important moment in the product was unreachable by keyboard
  // without tabbing through the entire page behind it.
  focusRestore =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;

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
    warn.textContent = '';
    const tag = document.createElement('span');
    tag.className = 'chip chip--caution';
    tag.textContent = 'Hard gate';
    warn.append(tag, document.createTextNode(
      ' Capybaras will not remember it, and no number of past approvals will let it through on its own.',
    ));
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

  // Land on the task: the phrase field when one is required, otherwise the safe
  // action. Enter still holds on from anywhere in the card, including here.
  const landing = request.requiresTypedConfirmation && request.confirmationPhrase
    ? $('card-phrase-input')
    : $('btn-hold');
  if (landing) landing.focus();

  log(`approval required: ${request.headline} [${request.tier}]`);
}

/** Where focus was before the card opened, so it can go back after. */
let focusRestore = null;

function hideCard() {
  $('card').hidden = true;
  current = null;
  document.body.classList.remove('is-locked');
  if (focusRestore && document.contains(focusRestore)) focusRestore.focus();
  focusRestore = null;
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
// confirmation field. Escape is the same answer. The reflex cannot approve; it
// can only stop. Tab is held inside the dialog while it is open.
const CARD_FOCUSABLE = 'button:not(:disabled), input, summary, [href]';

document.addEventListener('keydown', (event) => {
  const card = $('card');
  if (card.hidden) return;

  if (event.key === 'Escape' || event.key === 'Enter') {
    event.preventDefault();
    answer('deny');
    return;
  }

  if (event.key !== 'Tab') return;
  const items = [...card.querySelectorAll(CARD_FOCUSABLE)].filter((el) => el.offsetParent !== null);
  if (items.length === 0) return;
  const first = items[0];
  const last = items[items.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
});

/* ------------------------------------------------------------------ */
/* The explanation overlay                                             */
/* ------------------------------------------------------------------ */

/**
 * M5 section 9: the person is about to hand this app access to a paid account,
 * and they should understand that before they connect rather than after.
 *
 * The overlay follows the card's conventions exactly — focus moves in, Escape
 * closes it, focus returns, and the page behind it holds still — but it is
 * opened on demand only. It is never opened on its own, so it can never appear
 * between someone and the thing they were doing.
 */
const HELP_FOCUSABLE = 'button:not(:disabled), a[href], summary, [href]';

/** Where focus was before the overlay opened, so it can go back after. */
let helpFocusRestore = null;

function openHelp(trigger) {
  const help = $('help');
  if (!help || !help.hidden) return;
  // Prefer the control that opened it, so focus always comes back to where the
  // person was — whether they used the mouse, the keyboard, or an assistive
  // device. Falling back to whatever was focused covers any other caller.
  helpFocusRestore =
    trigger instanceof HTMLElement
      ? trigger
      : document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
  help.hidden = false;
  document.body.classList.add('is-locked');
  // Land on the one action, so the overlay is usable from the keyboard the
  // moment it appears.
  const close = $('btn-help-close');
  if (close) close.focus();
  log('opened the OpenRouter explanation');
}

function closeHelp() {
  const help = $('help');
  if (!help || help.hidden) return;
  help.hidden = true;
  // Only unlock the page if the approval card is not also up.
  const card = $('card');
  if (!card || card.hidden) document.body.classList.remove('is-locked');
  if (helpFocusRestore && document.contains(helpFocusRestore)) helpFocusRestore.focus();
  helpFocusRestore = null;
}

for (const id of ['btn-help-connect', 'btn-help-models']) {
  const trigger = $(id);
  if (trigger) trigger.addEventListener('click', () => openHelp(trigger));
}
const helpCloseButton = $('btn-help-close');
if (helpCloseButton) helpCloseButton.addEventListener('click', closeHelp);

// Escape closes it, and Tab is held inside while it is open. The card owns the
// keyboard while a decision is pending, so this stands down then.
document.addEventListener('keydown', (event) => {
  const help = $('help');
  if (!help || help.hidden) return;
  const card = $('card');
  if (card && !card.hidden) return;

  if (event.key === 'Escape') {
    event.preventDefault();
    closeHelp();
    return;
  }

  if (event.key !== 'Tab') return;
  const items = [...help.querySelectorAll(HELP_FOCUSABLE)].filter((el) => el.offsetParent !== null);
  if (items.length === 0) return;
  const first = items[0];
  const last = items[items.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
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

  // The sign belongs to the STATE, not to a character (BRAND.md section 4):
  // whoever needs you raises it, and it is drawn identically every time so the
  // single most important moment is recognised rather than re-read.
  const sign = document.createElementNS(SVG_NS, 'svg');
  sign.setAttribute('viewBox', '0 0 24 24');
  sign.setAttribute('class', 'agent__sign');
  sign.setAttribute('aria-hidden', 'true');
  const board = document.createElementNS(SVG_NS, 'rect');
  board.setAttribute('x', '2');
  board.setAttribute('y', '2');
  board.setAttribute('width', '20');
  board.setAttribute('height', '11');
  board.setAttribute('rx', '2.5');
  board.setAttribute('fill', 'currentColor');
  const post = document.createElementNS(SVG_NS, 'rect');
  post.setAttribute('x', '11');
  post.setAttribute('y', '12');
  post.setAttribute('width', '2');
  post.setAttribute('height', '10');
  post.setAttribute('rx', '1');
  post.setAttribute('fill', 'currentColor');
  sign.append(board, post);

  const name = document.createElement('span');
  name.className = 'agent__name';
  name.textContent = agent.label;

  // The state word lives in its own element so it can be updated in place
  // without erasing the sign glyph beside it.
  const state = document.createElement('span');
  state.className = 'agent__state';
  const stateText = document.createElement('span');
  stateText.className = 'agent__state-text';
  state.append(sign, stateText);

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

  const needing = [];
  const working = [];
  for (const agent of agents) {
    const el = host.querySelector(`[data-agent="${agent.id}"]`);
    if (!el) continue;
    const label = STATE_LABEL[agent.state] ?? agent.state;
    el.dataset.state = agent.state;
    el.querySelector('.agent__state-text').textContent = label;
    el.setAttribute('aria-label', `${agent.label}, ${agent.job}: ${label}`);
    if (agent.state === 'needs-you') needing.push(agent.label);
    if (agent.state === 'working') working.push(agent.label);
  }

  // The same state in words, so the rail never depends on decoding a colour.
  // Every line here describes a state the sidecar actually reported.
  const summary = $('herd-summary');
  if (summary) {
    if (needing.length === 1) summary.textContent = `${needing[0]} needs you.`;
    else if (needing.length > 1) summary.textContent = `${needing.length} capybaras need you.`;
    else if (working.length === 1) summary.textContent = `${working[0]} is working.`;
    else if (working.length > 1) summary.textContent = `${working.length} capybaras are working.`;
    else summary.textContent = 'All quiet. The capybaras are dozing.';
  }

  // The wave under the header runs only while real work is happening — the
  // same fact as the working capybaras, shown where it is always in view.
  const busy = $('busy-line');
  if (busy) busy.hidden = working.length === 0;
}

/* ------------------------------------------------------------------ */
/* The meter                                                          */
/* ------------------------------------------------------------------ */

/**
 * What this session has spent.
 *
 * Every figure comes from the shell's own count of real provider responses.
 * Nothing here is estimated: a price table times a token count would produce a
 * number that disagrees with the invoice, and a meter that disagrees with the
 * invoice is worse than no meter.
 *
 * The credit figure is the one number that is NOT this app's own spend, so it
 * says which it is. An account balance is shared with anything else using the
 * same account; presenting it as this app's budget would be a quiet lie.
 */
function renderUsage(snapshot) {
  const dl = $('usage-figures');
  if (!dl || !snapshot) return;

  dl.textContent = '';
  dl.append(...row('model calls', String(snapshot.calls)));
  dl.append(...row('tokens', String(snapshot.total_tokens)));
  dl.append(...row('spent', snapshot.cost_display, 'readout__figure'));

  if (snapshot.credit) {
    const scope = snapshot.credit.scope === 'key' ? 'this key' : 'whole account';
    dl.append(
      ...row(
        `left (${scope})`,
        snapshot.credit.remaining_display,
        null,
        snapshot.credit.remaining_usd <= 0 ? 'danger' : null,
      ),
    );
  }

  // The explanation says the same figure the meter says, and it says NOTHING at
  // all until the shell has read a real balance. A guessed number in an
  // explanation about money is worse than no number, so an unknown balance hides
  // the line rather than filling it in.
  const live = $('help-live');
  const liveBalance = $('help-live-balance');
  if (live && liveBalance) {
    const credit = snapshot.credit;
    const display =
      credit && typeof credit.remaining_display === 'string' ? credit.remaining_display : '';
    if (display.length > 0) {
      liveBalance.textContent = display;
      const liveScope = $('help-live-scope');
      if (liveScope) {
        liveScope.textContent = credit.scope === 'key' ? 'this key' : 'your OpenRouter account';
      }
      live.hidden = false;
    } else {
      live.hidden = true;
    }
  }

  // The header figure is the same number, not a second opinion about it.
  const mini = $('meter-mini-value');
  if (mini) mini.textContent = String(snapshot.cost_display ?? '—');

  // The skeleton has been replaced by the real thing.
  dl.setAttribute('aria-busy', 'false');
}

async function refreshUsage() {
  if (!invoke) return;
  try {
    renderUsage(await invoke('usage_status'));
  } catch (error) {
    const dl = $('usage-figures');
    if (dl) {
      dl.textContent = '';
      dl.append(...row('error', String(error), 'bad'));
    }
  }
}

/* ------------------------------------------------------------------ */
/* Remembered choices                                                 */
/* ------------------------------------------------------------------ */

/**
 * The grants that are currently in force.
 *
 * Rendered with text, never HTML: the target is a filename or a resource name,
 * which means it is whatever its author wrote. That rule is the same one the
 * approval card follows, and it applies here for the same reason.
 */
function renderGrants(grants) {
  const list = $('grants');
  const empty = $('grants-empty');
  if (!list || !Array.isArray(grants)) return;

  list.textContent = '';
  for (const grant of grants) {
    const item = document.createElement('li');
    item.className = 'grants__item';

    const text = document.createElement('span');
    text.textContent = grant.description ?? `${grant.tool} on ${grant.target}`;

    const forget = document.createElement('button');
    forget.className = 'btn btn--secondary';
    forget.textContent = 'Forget';
    forget.addEventListener('click', async () => {
      forget.disabled = true;
      log(`forgetting ${grant.id}`);
      try {
        // The refreshed list comes back as an event, so nothing is removed here
        // by hand: the display follows the store rather than guessing.
        await invoke('revoke_grant', { id: grant.id });
      } catch (error) {
        forget.disabled = false;
        log(`could not forget that: ${error}`);
      }
    });

    item.append(text, forget);
    list.append(item);
  }

  if (empty) empty.hidden = grants.length > 0;
}

async function refreshGrants() {
  if (!invoke) return;
  try {
    await invoke('list_grants');
  } catch (error) {
    log(`could not read the remembered choices: ${error}`);
  }
}

/* ------------------------------------------------------------------ */
/* The first reply                                                     */
/* ------------------------------------------------------------------ */

/** The model list as the shell last sent it, so a change can update the detail. */
let catalogModels = [];

/**
 * The detail link, but only if it is the link this product builds.
 *
 * The shell already builds `https://openrouter.ai/<id>` locally, never from the
 * API's `links.details`. This is the second check: the interface refuses to put
 * an href in the document unless it points at that fixed host. Belt and braces on
 * the one field that becomes an attribute rather than text.
 */
function safeModelLink(value) {
  const link = typeof value === 'string' ? value : '';
  return link.startsWith('https://openrouter.ai/') ? link : '';
}

/**
 * Which API serves the chosen model, and where to read about it.
 *
 * Both are remote-derived -- the provider from the id prefix, the link from the
 * id -- and both are rendered as text, never as markup (T3/T4). The link's href
 * is the single attribute set from that data, and it is checked first.
 */
function renderModelDetail() {
  const select = $('model-choice');
  const detail = $('model-detail');
  if (!select || !detail) return;

  const chosen = catalogModels.find((model) => String(model.id ?? '') === select.value);
  detail.textContent = '';
  if (!chosen) return;

  const provider = String(chosen.provider ?? '');
  if (provider.length > 0) {
    detail.append(document.createTextNode(`API: ${provider}`));
  }

  const href = safeModelLink(chosen.link);
  if (href) {
    if (detail.textContent.length > 0) detail.append(document.createTextNode(' \u00b7 '));
    const link = document.createElement('a');
    link.href = href;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.textContent = 'Open on OpenRouter';
    detail.append(link);
  }
}

/**
 * The model list, as the shell fetched it.
 *
 * THE NAME IS REMOTE TEXT. It comes from OpenRouter and is rendered here, which
 * makes it attacker-influenced content in a trusted position — the same shape as
 * the approval card (docs/threat-model.md, T4). So every option is written with
 * `textContent` and never as markup. The id is opaque and is sent back verbatim.
 */
function renderModels(payload) {
  const select = $('model-choice');
  const state = $('models-state');
  if (!select || !payload || !Array.isArray(payload.models)) return;

  catalogModels = payload.models;
  select.textContent = '';
  for (const model of payload.models) {
    const option = document.createElement('option');
    option.value = String(model.id ?? '');
    option.textContent = String(model.name ?? model.id ?? '');
    select.append(option);
  }
  // The shell chose the default over the real list, so the interface never has to
  // guess which model is the sensible first pick.
  if (payload.default) select.value = payload.default;
  modelsLoaded = true;
  // The API and a link for the chosen model, so picking one is informed.
  renderModelDetail();

  if (state) {
    const count = payload.models.length;
    state.textContent = count === 1 ? '1 model available.' : `${count} models available.`;
    state.className = 'hint';
  }
  log(`models loaded: ${payload.models.length}`);
}

function showModelsError(message) {
  const state = $('models-state');
  if (state) {
    state.textContent = String(message ?? 'the model list could not be loaded');
    state.className = 'hint bad';
  }
  log(`model list failed: ${message}`);
}

async function loadModels() {
  if (!invoke) return;
  try {
    // The list arrives as an event; only the dispatch is awaited.
    await invoke('fetch_models');
  } catch (error) {
    showModelsError(String(error));
  }
}

/** The reply, or a plain-words failure. Never both on screen at once. */
function showReply(text, model) {
  const box = $('ask-reply');
  const error = $('ask-error');
  const button = $('btn-send');
  if (error) {
    error.hidden = true;
    error.textContent = '';
  }
  if (box) {
    box.hidden = false;
    // Model output: text only, always. The one place that rule is most tempting
    // to break is the one place it matters most.
    box.textContent = String(text ?? '');
  }
  if (button) {
    button.disabled = false;
    button.removeAttribute('aria-busy');
  }
  log(`reply from ${model}`);
}

function showMessageError(message) {
  const box = $('ask-reply');
  const error = $('ask-error');
  const button = $('btn-send');
  if (box) {
    box.hidden = true;
    box.textContent = '';
  }
  if (error) {
    error.hidden = false;
    error.textContent = '';
    // A calm, specific, readable failure -- never a raw technical string as the
    // primary experience. The chip says what happened; the sentence says what to
    // do about it.
    const tag = document.createElement('span');
    tag.className = 'chip chip--danger';
    tag.textContent = 'Not sent';
    error.append(tag, document.createTextNode(` ${String(message ?? 'something went wrong')}`));
  }
  if (button) {
    button.disabled = false;
    button.removeAttribute('aria-busy');
  }
  log(`message failed: ${message}`);
}

const sendButton = $('btn-send');
if (sendButton) {
  sendButton.addEventListener('click', async () => {
    const prompt = $('ask-prompt').value;
    const model = $('model-choice').value;
    const error = $('ask-error');
    const box = $('ask-reply');
    if (error) {
      error.hidden = true;
      error.textContent = '';
    }
    if (box) {
      box.hidden = true;
      box.textContent = '';
    }

    // Disabled while in flight: a second click would be a second billed call.
    sendButton.disabled = true;
    sendButton.setAttribute('aria-busy', 'true');
    try {
      await invoke('send_message', { prompt, model });
    } catch (reason) {
      // A refusal made before anything was sent — not connected, nothing typed.
      showMessageError(String(reason));
    }
  });
}

const modelsButton = $('btn-models');
if (modelsButton) {
  modelsButton.addEventListener('click', loadModels);
}

// Picking a different model updates the API and the detail link beside the list.
const modelChoice = $('model-choice');
if (modelChoice) {
  modelChoice.addEventListener('change', renderModelDetail);
}

/* ------------------------------------------------------------------ */
/* Events pushed from the shell                                       */
/* ------------------------------------------------------------------ */

/**
 * One satisfied nod for the capybara who raised the sign.
 *
 * The confirmation movement BRAND.md section 7 spends its fun budget on. It is
 * driven by a real resolution -- the sign coming down -- and does nothing at all
 * if no one was holding it. The class is removed afterwards so the animation can
 * run again on the next answer.
 */
function nodOnce() {
  const herd = $('herd');
  const holder = herd ? herd.querySelector("[data-state='needs-you']") : null;
  if (!holder) return;
  holder.classList.remove('is-settling');
  // Reading offsetWidth would force a reflow just to restart an animation; the
  // timeout is cheaper and the interval between answers is human-scale anyway.
  holder.classList.add('is-settling');
  setTimeout(() => holder.classList.remove('is-settling'), 700);
}

if (listen) {
  listen('capybaras://approval-required', (e) => showCard(e.payload.id, e.payload.request));
  listen('capybaras://approval-resolved', (e) => {
    log(`resolved: ${e.payload.id} → ${e.payload.outcome}`);
    nodOnce();
  });
  listen('capybaras://action-proceeded', (e) =>
    log(`proceeded: ${e.payload.id} (${e.payload.tier}) — ${e.payload.because ?? ''}`),
  );
  listen('capybaras://action-dry-run', (e) => log(`dry run: ${e.payload.headline}`));
  listen('capybaras://agents', (e) => renderHerd(e.payload.agents));
  // Every model call reports itself, so the meter is pushed rather than polled.
  listen('capybaras://usage', (e) => renderUsage(e.payload));
  // The first reply: the model list as fetched, and the one answer it produces.
  listen('capybaras://models', (e) => renderModels(e.payload));
  listen('capybaras://models-failed', (e) => showModelsError(e.payload.message));
  listen('capybaras://reply', (e) => showReply(e.payload.text, e.payload.model));
  listen('capybaras://message-failed', (e) => showMessageError(e.payload.message));
  // The remembered choices are revocable here, because a permission you cannot
  // withdraw is not one you really gave.
  listen('capybaras://grants', (e) => renderGrants(e.payload.grants));
  listen('capybaras://protocol-error', (e) => log(`refused: ${e.payload.message}`));
  listen('capybaras://ready', (e) => log(`sidecar ready (protocol ${e.payload.protocol})`));
  listen('capybaras://connect-waiting', (e) => {
    log(`waiting for your browser to come back (port ${e.payload.port})`);
    // The person has just been sent to their browser. If the panel keeps saying
    // "Not connected yet." they cannot tell the sign-in is running at all -- and
    // until now the only place it was said was the log below, which they never read.
    setConnectState(
      'A browser window has opened. Sign in to OpenRouter there, then come back to this window.',
    );
  });
  listen('capybaras://connect-connected', (e) => {
    log(`connected — a key is stored as "${e.payload.credentialName}" in Windows`);
    refreshConnect();
    // Straight on to the model list: reaching a first reply must not depend on
    // knowing that a "Load models" button exists.
    loadModels();
  });
  listen('capybaras://connect-failed', async (e) => {
    log(`sign-in failed (${e.payload.reason}): ${e.payload.detail}`);
    // refreshConnect puts the button back; it also restores "Not connected yet.",
    // so the reason is set AFTER it, or it would be overwritten by the generic line.
    await refreshConnect();
    // The reason has to reach the panel. A sign-in that fails silently reads as
    // "the button did nothing". Choosing not to authorize is not an error, so it
    // keeps the neutral colour; everything else is a problem worth noticing.
    setConnectState(
      String(e.payload.detail ?? 'the sign-in did not finish; try again'),
      e.payload.reason === 'denied' ? undefined : 'bad',
    );
  });
} else {
  log('not running inside Tauri — the card cannot be driven from here');
}

/* ------------------------------------------------------------------ */
/* Test harness — no agent is wired up yet, so propose by hand        */
/* ------------------------------------------------------------------ */

/** Whether the model list has been fetched once already. */
let modelsLoaded = false;

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
// Poll only while the window is visible: a hidden window has nothing to update,
// and looking again the moment it comes back is what keeps it honest.
setInterval(() => {
  if (document.visibilityState === 'visible') refreshStatus();
}, 5000);
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') refreshStatus();
});
refreshConnect();
refreshUsage();
refreshGrants();

const refreshGrantsButton = $('btn-refresh-grants');
if (refreshGrantsButton) {
  refreshGrantsButton.addEventListener('click', () => refreshGrants());
}

/* ------------------------------------------------------------------ */
/* Navigation: where you are                                           */
/* ------------------------------------------------------------------ */

/**
 * Which section the reader is in.
 *
 * Deliberately position-based rather than an IntersectionObserver: the herd
 * lives in a sticky rail, and a sticky element never stops intersecting the
 * viewport, so an observer would report it as current for the whole page. The
 * rail's own sections are therefore handled separately -- "Herd" is current when
 * no workspace section has been reached yet, which is exactly when it is true.
 *
 * It is progressive enhancement: without it the links still work, there is just
 * no highlighted one.
 */
(function markCurrentSection() {
  const links = [...document.querySelectorAll('.nav__link')];
  if (links.length === 0) return;

  const pairs = links
    .map((link) => ({ link, target: document.querySelector(link.getAttribute('href')) }))
    .filter((pair) => pair.target);
  if (pairs.length === 0) return;

  const inWorkspace = pairs.filter((pair) => !pair.target.closest('.rail'));
  const fallback = pairs.find((pair) => pair.target.closest('.rail')) ?? pairs[0];

  let queued = 0;
  function update() {
    queued = 0;
    // Anything whose top has passed under the sticky header counts as reached.
    // The last one reached in document order is where the reader is.
    let active = null;
    for (const pair of inWorkspace) {
      if (pair.target.getBoundingClientRect().top <= 104) active = pair;
    }
    if (!active) active = fallback;
    for (const { link } of pairs) link.removeAttribute('aria-current');
    active.link.setAttribute('aria-current', 'true');
  }

  const schedule = () => {
    if (queued) return;
    queued =
      typeof requestAnimationFrame === 'function'
        ? requestAnimationFrame(update)
        : setTimeout(update, 100);
  };

  window.addEventListener('scroll', schedule, { passive: true });
  window.addEventListener('resize', schedule);
  update();
})();
