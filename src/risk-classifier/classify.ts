/**
 * The risk classifier.
 *
 * One question, asked before every action the agent proposes:
 *   *how much should we interrupt the human?*
 *
 * Three rules govern everything here:
 *
 *   1. CLASSIFICATION IS BY THE SYSTEM, NOT THE MODEL. A model that has been
 *      manipulated must not be able to argue its way to a lower tier.
 *   2. ESCALATION ONLY. Rules may raise the tier. Nothing may lower it.
 *   3. UNKNOWN FAILS SAFE. An unrecognised tool is treated as needing consent,
 *      not as harmless.
 */

import type { RiskTier, Taint } from './tiers';
import { atLeast, escalate, raise } from './tiers';

export type { RiskTier, Taint };

/** Everything the classifier is allowed to consider. */
export interface ActionDescriptor {
  /** Dotted tool id, e.g. `fs.delete`, `exec`, `message.send`. */
  tool: string;
  /** Raw arguments, scanned for destructive patterns. */
  args?: Record<string, unknown>;
  /** How many objects this touches. Drives blast-radius escalation. */
  affectedCount?: number;
  /** Defaults to true for most tools; set false for anything irreversible. */
  reversible?: boolean;
  touchesSecrets?: boolean;
  involvesMoney?: boolean;
  changesSecurityConfig?: boolean;
  /** The action leaves this machine (network, email, message). */
  leavesMachine?: boolean;
  /**
   * Where the *motivation* for this action came from. `untrusted` means the
   * agent is acting on content it read from a page, file, or message rather
   * than on a direct human instruction.
   */
  taint?: Taint;
  /** Target is marked off-limits — e.g. production during a code freeze. */
  protectedTarget?: boolean;
}

export interface Classification {
  tier: RiskTier;
  /** Why, in plain language. Surfaced to the user; also our audit trail. */
  reasons: string[];
  /** May the UI offer "always allow this"? Never for untrusted motivation. */
  allowRemember: boolean;
  /** Hard gates require the user to type something, not just click. */
  requiresTypedConfirmation: boolean;
  /** Destructive argument patterns that were detected, if any. */
  blockedPatterns: string[];
}

/* ------------------------------------------------------------------ */
/* Baseline tiers per tool                                            */
/* ------------------------------------------------------------------ */

const BASE_TIERS: Readonly<Record<string, RiskTier>> = Object.freeze({
  // reading is free
  'fs.read': 'silent',
  'fs.list': 'silent',
  'fs.stat': 'silent',
  'web.fetch': 'silent',
  'web.search': 'silent',
  'clock.read': 'silent',

  // creating is cheap to undo
  'fs.create': 'notify',
  'fs.mkdir': 'notify',

  // changing or removing existing things needs a human
  'fs.write': 'confirm',
  'fs.edit': 'confirm',
  'fs.move': 'confirm',
  'fs.rename': 'confirm',
  'fs.delete': 'confirm',
  'fs.truncate': 'confirm',
  'db.delete': 'confirm',
  'db.update': 'confirm',
  exec: 'confirm',
  'message.send': 'confirm',
  'mail.send': 'confirm',
  'net.post': 'confirm',
  'net.put': 'confirm',
  'net.delete': 'confirm',

  // never without an explicit, typed decision
  'credentials.read': 'hard_gate',
  'credentials.write': 'hard_gate',
  spend: 'hard_gate',
  'payment.send': 'hard_gate',
  'config.security': 'hard_gate',
  'agent.selfModify': 'hard_gate',
});

/** Unknown tools are treated as needing consent, never as harmless. */
const UNKNOWN_TOOL_TIER: RiskTier = 'confirm';

/** Tools where untrusted motivation is disqualifying on its own. */
const TAINT_SENSITIVE_TOOLS: ReadonlySet<string> = new Set([
  'exec',
  'fs.delete',
  'fs.truncate',
  'db.delete',
  'net.post',
  'net.put',
  'net.delete',
  'message.send',
  'mail.send',
]);

/** Blast-radius thresholds. */
const MASS_OPERATION = 10;
const CATASTROPHIC_OPERATION = 100;

/* ------------------------------------------------------------------ */
/* Destructive argument patterns                                      */
/* ------------------------------------------------------------------ */

const DESTRUCTIVE_PATTERNS: ReadonlyArray<{ name: string; re: RegExp }> = [
  { name: 'recursive force delete', re: /\brm\s+-(?=[a-z]*r)(?=[a-z]*f)[a-z]+/i },
  { name: 'recursive force delete', re: /\brm\s+-(?=[a-z]*f)(?=[a-z]*r)[a-z]+/i },
  { name: 'windows force delete', re: /\bdel\s+\/[fsq]/i },
  { name: 'windows recursive rmdir', re: /\brmdir\s+\/s/i },
  { name: 'disk format', re: /\b(?:format|mkfs)(?:\.\w+)?\b/i },
  { name: 'raw device write', re: />\s*\/dev\/(?:sd|nvme|hd)/i },
  { name: 'database drop', re: /\bdrop\s+(?:table|database|schema)\b/i },
  { name: 'database truncate', re: /\btruncate\s+table\b/i },
  { name: 'destructive git reset', re: /\bgit\s+reset\s+--hard\b/i },
  { name: 'destructive git clean', re: /\bgit\s+clean\s+-[a-z]*f/i },
  { name: 'machine shutdown', re: /\b(?:shutdown|halt|poweroff)\b/i },
  { name: 'key or account destruction', re: /\b(?:aws|gcloud|az)\b.*\b(?:delete|terminate|destroy)\b/i },
];

function collectStrings(value: unknown, out: string[] = [], depth = 0): string[] {
  if (depth > 6 || out.length > 200) return out;
  if (typeof value === 'string') {
    out.push(value);
  } else if (Array.isArray(value)) {
    for (const item of value) collectStrings(item, out, depth + 1);
  } else if (value !== null && typeof value === 'object') {
    for (const item of Object.values(value as Record<string, unknown>)) {
      collectStrings(item, out, depth + 1);
    }
  }
  return out;
}

/* ------------------------------------------------------------------ */
/* The classifier                                                     */
/* ------------------------------------------------------------------ */

export function classify(action: ActionDescriptor): Classification {
  const reasons: string[] = [];
  const blockedPatterns: string[] = [];

  const base = BASE_TIERS[action.tool] ?? UNKNOWN_TOOL_TIER;
  if (!(action.tool in BASE_TIERS)) {
    reasons.push(`Unrecognised action "${action.tool}" — treated as needing your consent.`);
  }

  // Escalation only. `tier` starts at the baseline and may only go up.
  let tier: RiskTier = base;

  /**
   * Record a hazard, and escalate to at least `floor`.
   *
   * The two halves are deliberately separate. The tier decides *whether* we
   * interrupt; the reasons decide whether the human can actually understand
   * what they are approving. An action carrying four hazards must list four
   * reasons — reporting only "off-limits" would hide the fact that 1,200 rows
   * are about to go. That failure mode is the reason this product exists.
   */
  const hazard = (floor: RiskTier, why: string): void => {
    reasons.push(why);
    tier = escalate(tier, floor);
  };

  /* --- hard disqualifiers --------------------------------------- */

  if (action.protectedTarget === true) {
    hazard('hard_gate', 'This target is marked off-limits.');
  }

  if (action.touchesSecrets === true) {
    hazard('hard_gate', 'This would reach your saved passwords or keys.');
  }

  if (action.involvesMoney === true) {
    hazard('hard_gate', 'This would spend money.');
  }

  if (action.changesSecurityConfig === true) {
    hazard('hard_gate', 'This would change your security settings.');
  }

  const strings = action.args ? collectStrings(action.args) : [];
  for (const { name, re } of DESTRUCTIVE_PATTERNS) {
    if (strings.some((s) => re.test(s))) {
      if (!blockedPatterns.includes(name)) blockedPatterns.push(name);
    }
  }
  if (blockedPatterns.length > 0) {
    hazard('hard_gate', `This matches a destructive command pattern (${blockedPatterns.join(', ')}).`);
  }

  /* --- blast radius --------------------------------------------- */

  const count = action.affectedCount ?? 0;
  if (count > CATASTROPHIC_OPERATION) {
    hazard('hard_gate', `This would affect ${count} things at once.`);
  } else if (count > MASS_OPERATION) {
    hazard(raise(tier), `This would affect ${count} things at once.`);
  }

  /* --- reversibility -------------------------------------------- */

  if (action.reversible === false) {
    hazard(atLeast(tier, 'confirm'), 'This cannot be undone.');
    if (count > MASS_OPERATION) {
      hazard('hard_gate', 'This cannot be undone, and it affects many things.');
    }
  }

  /* --- leaving the machine -------------------------------------- */

  if (action.leavesMachine === true) {
    hazard(atLeast(tier, 'confirm'), 'This would reach outside your computer.');
  }

  /* --- motivation taint ----------------------------------------- */

  const taint: Taint = action.taint ?? 'trusted';
  const tainted = taint === 'untrusted' || taint === 'mixed';

  if (tainted) {
    hazard(atLeast(tier, 'confirm'), 'The instruction for this came from content the agent read, not from you.');
    if (taint === 'untrusted' && TAINT_SENSITIVE_TOOLS.has(action.tool)) {
      hazard('hard_gate', 'An untrusted source asked for something that can destroy or send.');
    }
  }

  /* --- what the UI is allowed to offer -------------------------- */

  const allowRemember =
    tier === 'confirm' &&
    taint === 'trusted' &&
    action.reversible !== false &&
    action.protectedTarget !== true;

  return {
    tier,
    reasons,
    allowRemember,
    requiresTypedConfirmation: tier === 'hard_gate',
    blockedPatterns,
  };
}

/** Convenience: does this action require a human decision before running? */
export function needsHuman(classification: Classification): boolean {
  return classification.tier === 'confirm' || classification.tier === 'hard_gate';
}
