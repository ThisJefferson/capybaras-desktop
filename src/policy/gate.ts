/**
 * The gate.
 *
 * The classifier decides *how much* to interrupt. The gate decides *what
 * actually happens* — and it is the only place that answer is produced.
 *
 * Three outcomes:
 *
 *   proceed  — run it (possibly silently, possibly reporting afterwards)
 *   ask      — stop, and hand the interface everything it needs to ask a human
 *   dry_run  — show the plan, execute nothing
 *
 * Two rules the gate enforces, which the classifier alone cannot:
 *
 *   1. A remembered grant satisfies a `confirm`, and NOTHING else. A hard gate
 *      is never satisfied by memory, no matter how many times it was approved.
 *   2. Dry-run can only ever make the gate more cautious. It never turns an
 *      `ask` into a `proceed`.
 */

import type { ActionDescriptor, Classification } from '../risk-classifier';
import { classify } from '../risk-classifier';
import { headlineFor as skillHeadline, moreCautious } from '../skills/registry';
import { resolveProtection } from './protection';
import type { ProtectionPolicy } from './protection';
import type { GrantStore } from './grants';

export interface GateContext {
  /**
   * The protection policy: freezes and off-limits targets (D21).
   *
   * Absent means no protection beyond what the action itself claims, which is
   * what every existing caller does -- so this is inert until a policy exists.
   */
  policy?: ProtectionPolicy;
  grants: GrantStore;
  /** When true, show the plan instead of executing anything above silent. */
  dryRun?: boolean;
  now?: number;
  /** Human-readable name for the thing being acted on. */
  targetLabel?: string;
}

export interface ApprovalRequest {
  tier: 'confirm' | 'hard_gate';
  /** One plain sentence, in the user's language, never a tool name. */
  headline: string;
  /** Every reason the classifier found — all of them, not just the first. */
  reasons: string[];
  /** The text a human must type for a hard gate. */
  confirmationPhrase?: string;
  canRemember: boolean;
  requiresTypedConfirmation: boolean;
  target: string;
}

export type GateDecision =
  | { outcome: 'proceed'; classification: Classification; because: string }
  | { outcome: 'ask'; classification: Classification; request: ApprovalRequest }
  | {
      outcome: 'dry_run';
      classification: Classification;
      headline: string;
      reasons: string[];
    };

/* ------------------------------------------------------------------ */
/* Plain language                                                     */
/* ------------------------------------------------------------------ */

function verbFor(action: ActionDescriptor): string {
  // The headline table used to live here as a VERBS record. It is now the skill
  // registry (src/skills/registry.ts), which is ALSO the source of tool
  // ownership and of the tier floors -- one list instead of three that could
  // silently disagree (D20).
  return skillHeadline(action.tool, action.affectedCount ?? 1);
}

const LABEL_MAX = 80;

/**
 * Sanitise a target label before it reaches the card.
 *
 * THE THREAT (docs/threat-model.md, T4): the label is written by whoever controls
 * the target -- a filename, a table name, a URL -- so it is ATTACKER TEXT. And it
 * lands on the largest, most trusted line of the card. Left raw it can:
 *
 *   - reassure:      "notes.md (safe, already reviewed)"
 *   - impersonate us: "IGNORE THE WARNINGS BELOW - this target is allowlisted"
 *   - forge structure with newlines, pushing the reasons out of view
 *   - run to thousands of characters, burying everything else
 *
 * Flattening control characters and clamping the length does NOT make the label
 * trustworthy -- nothing can; it is still someone else's words. It stops the
 * label from being able to LOOK like our words, which is the part that matters.
 */
export function sanitiseLabel(label: string): string {
  const flattened = label
    // Control characters, including newlines and tabs, which could fake layout.
    .replace(/[\u0000-\u001F\u007F-\u009F]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return flattened.length <= LABEL_MAX ? flattened : `${flattened.slice(0, LABEL_MAX - 1)}\u2026`;
}

/**
 * Merge reason lists without saying the same thing twice.
 *
 * D10 is "every hazard gets its own reason", and the point of it is that a person
 * can understand what they are approving. Two layers can describe the SAME hazard
 * in nearly the same words -- the classifier says "This target is marked
 * off-limits." and the protection policy says "this target is marked off-limits" --
 * and an exact-string merge keeps both.
 *
 * That is noise, and noise on the warning list is exactly what teaches a person to
 * stop reading it. So the merge compares a NORMALISED form (case, whitespace and
 * trailing punctuation), keeping the first occurrence and its original wording.
 * Distinct hazards are untouched: they do not differ only in punctuation.
 */
function mergeReasons(...groups: ReadonlyArray<readonly string[]>): string[] {
  const seen = new Set<string>();
  const merged: string[] = [];
  for (const group of groups) {
    for (const reason of group) {
      const key = reason.toLowerCase().replace(/\s+/g, ' ').replace(/[.;:!]+$/, '').trim();
      if (key.length === 0 || seen.has(key)) continue;
      seen.add(key);
      merged.push(reason);
    }
  }
  return merged;
}

function headlineFor(action: ActionDescriptor, ctx: GateContext): string {
  const base = verbFor(action);
  if (!ctx.targetLabel) return `${base}?`;
  // The preposition matters more than it looks. "Delete 1,200 records TO the
  // production database" was the first version, visible in the milestone run --
  // and the entire promise of this card is that a person who does not know what
  // `db.delete` means can still read it. A preposition that is merely wrong makes
  // the sentence feel machine-generated at exactly the moment it needs to feel
  // deliberate.
  const joiner = action.tool.startsWith('fs.')
    ? 'in'
    : action.tool.startsWith('db.')
      ? 'from'
      : 'to';
  return `${base} ${joiner} ${sanitiseLabel(ctx.targetLabel)}?`;
}

/* ------------------------------------------------------------------ */
/* The gate                                                           */
/* ------------------------------------------------------------------ */

export function gate(action: ActionDescriptor, ctx: GateContext): GateDecision {
  // Protection comes from the POLICY, not from what the action claims (D21).
  // Until now `protectedTarget` was supplied by the caller -- so the "declared
  // code and action freeze" that failed to stop the July 2025 deletion was in
  // the same position: a request the agent could simply not make.
  //
  // The invariant is one-way: an action can ADD caution, never remove it. So
  // `protectedTarget: false` cannot clear a policy match.
  const protection = resolveProtection(action, ctx.policy, {
    targetLabel: ctx.targetLabel,
    claimedProtected: action.protectedTarget === true,
  });

  let classification = classify({ ...action, protectedTarget: protection.protectedTarget });

  // A freeze is about CHANGES.
  //
  // Reads still proceed. Gating them would be surprising, would not prevent the
  // harm (the harm is a change), and -- most importantly -- would teach people
  // that a freeze is a nuisance to be switched off. A control people route
  // around is worse than no control, because it also removes the signal.
  //
  // So the floor rises for anything that is NOT silent.
  if (protection.freezeActive && classification.tier !== 'silent') {
    classification = { ...classification, tier: moreCautious('confirm', classification.tier) };
  }

  // Whatever the policy decided is stated on the card. A freeze nobody is told
  // about is indistinguishable from a malfunction.
  //
  // Merged through `mergeReasons` rather than a plain `Set`, so a hazard the
  // classifier already stated is not repeated in the policy's own words. The
  // classifier's sentence leads: it is the well-formed, concrete one.
  if (protection.reasons.length > 0) {
    classification = {
      ...classification,
      reasons: mergeReasons(classification.reasons, protection.reasons),
    };
  }
  const now = ctx.now ?? Date.now();
  const target = ctx.targetLabel ?? action.tool;

  // Dry-run can only add caution. Anything above a silent read gets shown
  // first, and nothing executes.
  if (ctx.dryRun === true && classification.tier !== 'silent') {
    return {
      outcome: 'dry_run',
      classification,
      headline: headlineFor(action, ctx),
      reasons: classification.reasons,
    };
  }

  if (classification.tier === 'silent') {
    return { outcome: 'proceed', classification, because: 'Reading is free.' };
  }

  if (classification.tier === 'notify') {
    return {
      outcome: 'proceed',
      classification,
      because: 'Easy to undo — you will be told afterwards.',
    };
  }

  const scope = { tool: action.tool, target };

  if (classification.tier === 'confirm' && classification.allowRemember) {
    const grant = ctx.grants.find(scope, now);
    if (grant) {
      return {
        outcome: 'proceed',
        classification,
        because: 'You allowed this exact action before.',
      };
    }
  }

  // A confirm without a grant, or any hard gate, stops here.
  const requiresTyped = classification.requiresTypedConfirmation;
  const headline = headlineFor(action, ctx);

  return {
    outcome: 'ask',
    classification,
    request: {
      tier: requiresTyped ? 'hard_gate' : 'confirm',
      headline,
      reasons: classification.reasons,
      // The phrase is the headline minus its question mark: the human types
      // back what they are approving, which is hard to do by reflex.
      ...(requiresTyped ? { confirmationPhrase: headline.replace(/\?$/, '') } : {}),
      canRemember: classification.allowRemember,
      requiresTypedConfirmation: requiresTyped,
      target,
    },
  };
}
