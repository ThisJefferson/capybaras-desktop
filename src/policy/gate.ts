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
import { headlineFor as skillHeadline } from '../skills/registry';
import type { GrantStore } from './grants';

export interface GateContext {
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

function headlineFor(action: ActionDescriptor, ctx: GateContext): string {
  const base = verbFor(action);
  if (!ctx.targetLabel) return `${base}?`;
  const joiner = action.tool.startsWith('fs.') ? 'in' : 'to';
  return `${base} ${joiner} ${ctx.targetLabel}?`;
}

/* ------------------------------------------------------------------ */
/* The gate                                                           */
/* ------------------------------------------------------------------ */

export function gate(action: ActionDescriptor, ctx: GateContext): GateDecision {
  const classification = classify(action);
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
