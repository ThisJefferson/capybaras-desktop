/**
 * Risk tiers — the vocabulary the whole product is built on.
 *
 * The classifier answers one question: *how much should we interrupt the human
 * before this happens?* Four answers, ordered. Nothing else.
 */

export type RiskTier = 'silent' | 'notify' | 'confirm' | 'hard_gate';

/**
 * Where the *motivation* for an action came from.
 *
 * `trusted`   — the human asked for it directly
 * `untrusted` — it came from a page, file, or message the agent read
 * `mixed`     — some combination; treated as untrusted for escalation
 */
export type Taint = 'trusted' | 'untrusted' | 'mixed';

/** Ordinal ranking. Higher = more interruption. */
export const TIER_ORDER: Readonly<Record<RiskTier, number>> = Object.freeze({
  silent: 0,
  notify: 1,
  confirm: 2,
  hard_gate: 3,
});

export const TIER_LABEL: Readonly<Record<RiskTier, string>> = Object.freeze({
  silent: 'runs quietly',
  notify: 'runs, then tells you',
  confirm: 'stops and asks you',
  hard_gate: 'needs a typed confirmation',
});

/** Human-facing severity, used by the UI to pick a colour. */
export type Severity = 'calm' | 'mild' | 'attention' | 'stop';

export function severityOf(tier: RiskTier): Severity {
  switch (tier) {
    case 'silent':
      return 'calm';
    case 'notify':
      return 'mild';
    case 'confirm':
      return 'attention';
    case 'hard_gate':
      return 'stop';
  }
}

/** The stricter of two tiers. */
export function escalate(a: RiskTier, b: RiskTier): RiskTier {
  return TIER_ORDER[a] >= TIER_ORDER[b] ? a : b;
}

/**
 * One step stricter. Used by rules that say "at least one tier up" — for
 * example, an action whose input came from an untrusted page.
 */
export function raise(tier: RiskTier): RiskTier {
  switch (tier) {
    case 'silent':
      return 'notify';
    case 'notify':
      return 'confirm';
    case 'confirm':
      return 'hard_gate';
    case 'hard_gate':
      return 'hard_gate';
  }
}

/** Minimum tier required by "at least this strict" rules. */
export function atLeast(tier: RiskTier, floor: RiskTier): RiskTier {
  return escalate(tier, floor);
}
