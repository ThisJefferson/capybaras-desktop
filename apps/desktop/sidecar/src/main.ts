/**
 * The Capybaras sidecar -- protocol v1.
 *
 * Runs the policy layer and holds a conversation with the shell. See
 * `docs/protocol.md`.
 *
 * Two things to know before editing:
 *
 * 1. **stdout is the protocol channel.** Human-readable text must NEVER be
 *    written to stdout -- it would corrupt the stream. Diagnostics go to
 *    `sidecar.log` in the state directory.
 * 2. **An action at `confirm` or `hard_gate` cannot proceed without an
 *    `approval.answer`.** There is deliberately no other path. Each of the
 *    five rules in `docs/protocol.md` is enforced here, and each has a test.
 */

import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { gate, GrantStore, describeGrant, loadGrants, saveGrants, loadProtectionPolicy, withSelfProtection, policyIsUserWritable } from '../../../../src/policy/index';
import { readFileSync, existsSync, accessSync, constants } from 'node:fs';
import type { ApprovalRequest, GateContext, LoadResult } from '../../../../src/policy/index';
import { Herd } from './agents';
import type { ActionDescriptor, Classification } from '../../../../src/risk-classifier/index';

const PROTOCOL = 1;

const started = Date.now();
const argv = process.argv.slice(2);
const stateDir = argv.find((a) => a.startsWith('--state='))?.split('=')[1];

/* ------------------------------------------------------------------ */
/* Logging: the file is for humans, stdout is for the protocol         */
/* ------------------------------------------------------------------ */

let logFile: string | null = null;
if (stateDir) {
  try {
    mkdirSync(stateDir, { recursive: true });
    logFile = join(stateDir, 'sidecar.log');
    writeFileSync(logFile, '');
  } catch {
    // Logging must never be the thing that kills the sidecar.
  }
}

function say(message: string): void {
  if (!logFile) return;
  const line = `+${String(Date.now() - started).padStart(6)}ms  ${message}`;
  try {
    appendFileSync(logFile, `${line}\n`);
  } catch {
    // Ignored deliberately.
  }
}

/** Protocol output. Never write free text here. */
function emit(message: Record<string, unknown>): void {
  process.stdout.write(`${JSON.stringify({ v: PROTOCOL, ...message })}\n`);
}

/* ------------------------------------------------------------------ */
/* Boot                                                                */
/* ------------------------------------------------------------------ */

say(`sidecar up pid=${process.pid} node=${process.version}`);
say(`  execPath=${process.execPath}`);
say(`  cwd=${process.cwd()}`);

emit({ type: 'ready', pid: process.pid, node: process.version, protocol: PROTOCOL });

/**
 * The grant store, restored from disk.
 *
 * Persistence FAILS CLOSED: a missing, unreadable or corrupt file yields an
 * EMPTY store, which means everything asks again. A persistence bug must never
 * be able to grant authority. See src/policy/grant-file.ts.
 */
const grantsPath = stateDir ? join(stateDir, 'grants.json') : null;
const restored: LoadResult = grantsPath
  ? loadGrants(grantsPath)
  : { store: new GrantStore(), loaded: 0, rejected: 0 };
const grants = restored.store;

say(
  `grants: ${restored.loaded} restored, ${restored.rejected} refused` +
    (restored.problem ? ` -- ${restored.problem}` : ''),
);
if (restored.problem) {
  emit({ type: 'error', message: restored.problem });
}

/**
 * The protection policy: freezes and off-limits targets (D21).
 *
 * Read ONCE at boot, from a file this process never writes. That is the whole
 * point: protection comes from configuration, not from the action descriptor --
 * because a protection the protected agent supplies is not a protection. So
 * there is deliberately no code path here that creates or edits one.
 */
const policyPath = stateDir ? join(stateDir, 'policy.json') : null;
const protection = policyPath
  ? loadProtectionPolicy((p) => readFileSync(p, 'utf8'), policyPath)
  : { policy: {} };

// The freeze protects itself. The protection files are off-limits to the agent's
// own file tools whether or not an operator declared anything -- which is also a
// test of the design, because a mechanism that cannot say "do not touch the
// mechanism" is not really a mechanism.
const effectivePolicy = withSelfProtection(protection.policy, stateDir);

say(
  effectivePolicy.freeze?.active
    ? `protection: a freeze is IN FORCE${effectivePolicy.freeze.reason ? ` (${effectivePolicy.freeze.reason})` : ''}`
    : 'protection: no freeze in force',
);
if (protection.problem) {
  say(protection.problem);
  emit({ type: 'error', message: protection.problem });
}

// THE HONEST LIMIT, said out loud at every boot rather than buried in a comment.
// The agent runs as the same user as this process, so a policy file that user can
// write is one the agent can rewrite. No code in this repository can prevent
// that -- only the operating system can, with an ACL. So we detect it and say so,
// because a protection that is silently unenforceable is worse than one known to
// be unenforceable: people make decisions on it.
if (policyPath && existsSync(policyPath)) {
  const writable = policyIsUserWritable(policyPath, (p) => accessSync(p, constants.W_OK));
  if (writable) {
    const warning =
      `the protection policy at ${policyPath} is writable by this account, so the agent ` +
      'could rewrite it. Give it an ACL to make the freeze enforceable.';
    say(warning);
    emit({ type: 'error', message: warning });
  }
}

/** Persist after any change. Reports failure rather than throwing. */
function persistGrants(): void {
  if (!grantsPath) return;
  const problem = saveGrants(grantsPath, grants);
  if (problem) {
    say(problem);
    emit({ type: 'error', message: problem });
  }
}

/**
 * The herd: which specialist is doing what.
 *
 * Sent as WHOLE STATE rather than deltas, so a dropped message self-corrects on
 * the next one instead of leaving the display permanently wrong.
 */
const herd = new Herd();

function emitHerd(): void {
  emit({ type: 'agents.state', agents: herd.snapshot() });
}

// Send the initial picture straight away, so the interface never has to guess
// what the herd looks like before anything has happened.
emitHerd();

/** How long a completed action keeps its agent looking busy. Long enough to be
 * seen, short enough not to imply work is still happening. */
const WORK_VISIBLE_MS = 600;

/** Actions blocked and waiting for a human, keyed by correlation id. */
const pending = new Map<
  string,
  {
    action: ActionDescriptor;
    classification: Classification;
    request: ApprovalRequest;
    /** The specialist that raised the sign, so it can be lowered again. */
    owner: string;
  }
>();

let stopped = false;

function shutdown(reason: string): void {
  if (stopped) return;
  stopped = true;
  clearInterval(heartbeat);
  say(`GRACEFUL STOP RECEIVED (${reason}) -- handlers ran, exiting 0`);
  // Exit on the next tick so the log write above flushes.
  setTimeout(() => process.exit(0), 50);
}

const heartbeat = setInterval(() => {
  emit({ type: 'heartbeat', uptimeMs: Date.now() - started });
  say('heartbeat');
}, 5000);

function fail(id: string | undefined, message: string): void {
  emit({ type: 'error', ...(id ? { id } : {}), message });
  say(`error: ${message}`);
}

/* ------------------------------------------------------------------ */
/* The gate                                                            */
/* ------------------------------------------------------------------ */

function propose(id: string, action: ActionDescriptor, context?: Partial<GateContext>): void {
  const decision = gate(action, {
    grants,
    policy: effectivePolicy,
    now: Date.now(),
    targetLabel: context?.targetLabel,
    dryRun: context?.dryRun,
  });

  switch (decision.outcome) {
    case 'proceed': {
      // Show the owner briefly at work, then back to attentive. The action is
      // instantaneous from the sidecar's point of view, but the interface
      // should still be able to see that somebody did something.
      const owner = herd.beginWork(action.tool);
      emitHerd();
      setTimeout(() => {
        herd.release(owner);
        emitHerd();
      }, WORK_VISIBLE_MS);
      emit({
        type: 'action.proceeded',
        id,
        tier: decision.classification.tier,
        because: decision.because,
      });
      return;
    }

    case 'dry_run':
      emit({
        type: 'action.dry_run',
        id,
        headline: decision.headline,
        reasons: decision.reasons,
      });
      return;

    case 'ask': {
      if (pending.has(id)) {
        fail(id, 'an approval is already pending for that id');
        return;
      }
      // Rule 1: from here the action is BLOCKED. It stays blocked until an
      // explicit answer arrives. Nothing times it out into approval.
      // The owner raises the sign and holds it. This is the loudest state.
      const owner = herd.beginWork(action.tool);
      herd.needsYou(owner);
      emitHerd();

      pending.set(id, {
        action,
        classification: decision.classification,
        request: decision.request,
        owner,
      });

      emit({ type: 'approval.required', id, request: decision.request });
      say(`approval required id=${id} tier=${decision.request.tier}`);
      return;
    }
  }
}

function answer(id: string, decision: string): void {
  // Rule 4: an answer for something never proposed, or already resolved, is an
  // error -- never a queued approval waiting to be spent on whatever arrives
  // next. Resolving "the next pending approval" would let a stray or replayed
  // message approve an entirely different action.
  const entry = pending.get(id);
  if (!entry) {
    fail(id, 'no approval is pending with that id');
    return;
  }

  // Rule 2: a denial is final for this action.
  if (decision === 'deny') {
    pending.delete(id);
    herd.release(entry.owner);
    emitHerd();
    emit({ type: 'approval.resolved', id, outcome: 'denied' });
    say(`denied id=${id}`);
    return;
  }

  if (decision !== 'allow' && decision !== 'remember') {
    fail(id, `unrecognised decision: ${decision}`);
    return;
  }

  if (decision === 'remember') {
    // Rule 3, defence in depth. The classifier already refuses to offer this on
    // a hard gate, and the store refuses independently -- checked here a third
    // time so a malformed or hostile message cannot talk its way past either.
    if (!entry.request.canRemember || entry.request.tier === 'hard_gate') {
      fail(id, 'this action can never be remembered; it will always ask');
      return; // stays pending: the human must decide again, explicitly
    }

    const result = grants.record({
      scope: { tool: entry.action.tool, target: entry.request.target },
      classification: entry.classification,
      approvedBy: 'human',
      now: Date.now(),
    });

    if (!result.ok) {
      fail(id, result.reason);
      return; // stays pending
    }
    say(`remembered: ${describeGrant(result.grant)}`);
    persistGrants();
  }

  pending.delete(id);
  herd.release(entry.owner);
  emitHerd();
  emit({ type: 'approval.resolved', id, outcome: 'allowed' });
  emit({
    type: 'action.proceeded',
    id,
    tier: entry.request.tier,
    because: 'You approved it.',
  });
}

function listGrants(id?: string): void {
  emit({
    type: 'grants.listed',
    ...(id ? { id } : {}),
    grants: grants.list().map((grant) => ({
      ...grant,
      description: describeGrant(grant),
    })),
  });
}

/* ------------------------------------------------------------------ */
/* Input                                                              */
/* ------------------------------------------------------------------ */

function handleLine(line: string): void {
  let message: Record<string, unknown>;
  try {
    message = JSON.parse(line) as Record<string, unknown>;
  } catch {
    // Rule from the spec: a malformed line costs one message, never the stream.
    say(`unparseable input ignored: ${line.slice(0, 120)}`);
    return;
  }

  const id = typeof message.id === 'string' ? message.id : undefined;

  // The pre-M4 shutdown form is kept working on purpose: the supervision tests
  // assert that a graceful stop reaches this handler, and that guarantee must
  // survive this change rather than be quietly re-broken.
  if (message.cmd === 'shutdown' || message.type === 'shutdown') {
    shutdown('ipc');
    return;
  }

  if (message.v !== undefined && message.v !== PROTOCOL) {
    fail(id, `unsupported protocol version: ${String(message.v)}`);
    return;
  }

  switch (message.type) {
    case 'action.propose': {
      if (!id) {
        fail(undefined, 'action.propose requires an id');
        return;
      }
      const action = message.action as ActionDescriptor | undefined;
      if (!action || typeof action.tool !== 'string') {
        fail(id, 'action.propose requires an action with a tool');
        return;
      }
      propose(id, action, message.context as Partial<GateContext> | undefined);
      return;
    }

    case 'approval.answer': {
      if (!id) {
        fail(undefined, 'approval.answer requires an id');
        return;
      }
      answer(id, String(message.decision ?? ''));
      return;
    }

    case 'grants.list':
      listGrants(id);
      return;

    case 'grants.revoke': {
      const target = (message.scope as { id?: string } | undefined)?.id;
      if (!target) {
        fail(id, 'grants.revoke requires scope.id');
        return;
      }
      const removed = grants.revoke(target);
      say(removed ? `revoked ${target}` : `revoke: no such grant ${target}`);
      persistGrants();
      listGrants(id);
      return;
    }

    default:
      // Unknown types are ignored for forward compatibility -- reported, never fatal.
      fail(id, `unknown message type: ${String(message.type)}`);
      return;
  }
}

process.stdin.setEncoding('utf8');
let buffer = '';
process.stdin.on('data', (chunk: string) => {
  buffer += chunk;
  let newline: number;
  while ((newline = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    if (line) handleLine(line);
  }
});

process.stdin.on('end', () => shutdown('stdin closed'));
process.on('SIGTERM', () => shutdown('SIGTERM'));
process.on('SIGINT', () => shutdown('SIGINT'));
