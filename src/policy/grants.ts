/**
 * The grant store.
 *
 * A grant remembers that a human approved *this* action against *this* target.
 * It is the only mechanism by which an action can proceed without being asked
 * again — so the restrictions on it are the point, not an afterthought:
 *
 *   - A grant names ONE tool and ONE target. Never a folder, never a wildcard,
 *     never "all actions like this".
 *   - A grant expires. Thirty days by default.
 *   - A grant can never be created for a hard gate.
 *   - A grant can never be created for an action the classifier said is not
 *     rememberable — which excludes anything irreversible, anything with
 *     untrusted motivation, and anything touching a protected target.
 *   - Only a human may grant. The model has no code path that produces one.
 *
 * That last rule is enforced by the shape of `record()`: it demands an explicit
 * human provenance marker *and* a classification that already permits
 * remembering. A manipulated agent cannot call its way past the classifier,
 * because the classifier is consulted here too.
 */

import type { Classification } from '../risk-classifier';

/** What a grant is scoped to: exactly one action against exactly one target. */
export interface GrantScope {
  tool: string;
  /** A precise target — a path, a resource id, a recipient. Never a pattern. */
  target: string;
}

export interface Grant {
  id: string;
  tool: string;
  target: string;
  createdAt: number;
  expiresAt: number;
  grantedBy: 'human';
}

export type RecordResult =
  | { ok: true; grant: Grant }
  | { ok: false; reason: string };

/** Thirty days. Long enough to be useful, short enough to be revisited. */
export const DEFAULT_TTL_MS = 30 * 24 * 60 * 60 * 1000;

/**
 * Wildcards are rejected outright. A grant meaning "everything in Documents" is
 * the failure mode this product exists to prevent, so the store will not hold
 * one.
 */
const WILDCARD = /[*?]|^all$|^any$|^everything$/i;

export class GrantStore {
  private readonly grants = new Map<string, Grant>();

  /**
   * Record a human approval.
   *
   * Returns a refusal rather than throwing: a refusal is a normal outcome the
   * interface should be able to explain.
   */
  record(input: {
    scope: GrantScope;
    classification: Classification;
    approvedBy: 'human';
    now?: number;
    ttlMs?: number;
  }): RecordResult {
    const { scope, classification, approvedBy } = input;
    const now = input.now ?? Date.now();

    if (approvedBy !== 'human') {
      return { ok: false, reason: 'Only a person can allow this.' };
    }

    if (!scope.tool) {
      return { ok: false, reason: 'A remembered choice must name the action.' };
    }

    if (!scope.target || WILDCARD.test(scope.target)) {
      return {
        ok: false,
        reason: 'A remembered choice must name one specific target, not a pattern.',
      };
    }

    if (classification.tier === 'hard_gate') {
      return { ok: false, reason: 'This can never be remembered. It will always ask.' };
    }

    if (classification.tier !== 'confirm') {
      return { ok: false, reason: 'There is nothing to remember about this action.' };
    }

    if (!classification.allowRemember) {
      return {
        ok: false,
        reason:
          'This one cannot be remembered, because of how it was requested or what it would do.',
      };
    }

    const ttl = input.ttlMs ?? DEFAULT_TTL_MS;
    if (!Number.isFinite(ttl) || ttl <= 0) {
      return { ok: false, reason: 'A remembered choice must expire.' };
    }

    const grant: Grant = {
      id: globalThis.crypto.randomUUID(),
      tool: scope.tool,
      target: scope.target,
      createdAt: now,
      expiresAt: now + ttl,
      grantedBy: 'human',
    };

    this.grants.set(grant.id, grant);
    return { ok: true, grant };
  }

  /** Find a live grant for this exact action and target. */
  find(scope: GrantScope, now: number = Date.now()): Grant | undefined {
    for (const grant of this.grants.values()) {
      if (grant.tool === scope.tool && grant.target === scope.target && grant.expiresAt > now) {
        return grant;
      }
    }
    return undefined;
  }

  /** Everything currently allowed, soonest to expire first. */
  list(now: number = Date.now()): Grant[] {
    return [...this.grants.values()]
      .filter((g) => g.expiresAt > now)
      .sort((a, b) => a.expiresAt - b.expiresAt);
  }

  /** Revoke one grant. Returns whether it existed. */
  revoke(id: string): boolean {
    return this.grants.delete(id);
  }

  /** Revoke everything. The panic button. */
  revokeAll(): number {
    const n = this.grants.size;
    this.grants.clear();
    return n;
  }

  /** Drop expired grants. Returns how many were removed. */
  prune(now: number = Date.now()): number {
    let removed = 0;
    for (const [id, grant] of this.grants) {
      if (grant.expiresAt <= now) {
        this.grants.delete(id);
        removed += 1;
      }
    }
    return removed;
  }

  /**
   * Re-admit a grant loaded from disk.
   *
   * Applies the **same rules as `record`**, deliberately, rather than trusting
   * the file. A grants file is a file: it can be hand-edited, truncated, or
   * written by an older version of this software. Anything that would not have
   * been acceptable as a fresh human approval is not acceptable as a restored
   * one — persistence must not become a way to obtain authority that the
   * approval path would have refused.
   *
   * Expired grants are refused rather than resurrected. Returns whether the
   * candidate was admitted.
   */
  restore(candidate: unknown, now: number = Date.now()): boolean {
    if (candidate === null || typeof candidate !== 'object') return false;
    const g = candidate as Partial<Grant>;

    if (typeof g.id !== 'string' || g.id.length === 0) return false;
    if (typeof g.tool !== 'string' || g.tool.length === 0) return false;
    if (typeof g.target !== 'string' || g.target.length === 0) return false;
    if (WILDCARD.test(g.target)) return false;
    if (g.grantedBy !== 'human') return false;
    if (typeof g.createdAt !== 'number' || !Number.isFinite(g.createdAt)) return false;
    if (typeof g.expiresAt !== 'number' || !Number.isFinite(g.expiresAt)) return false;
    if (g.expiresAt <= now) return false;

    this.grants.set(g.id, {
      id: g.id,
      tool: g.tool,
      target: g.target,
      createdAt: g.createdAt,
      expiresAt: g.expiresAt,
      grantedBy: 'human',
    });
    return true;
  }

  /** Everything currently held, for persistence. Prunes expired entries first. */
  serialize(now: number = Date.now()): Grant[] {
    this.prune(now);
    return [...this.grants.values()];
  }
}

/** Plain-language summary of a grant, for the settings list. */
export function describeGrant(grant: Grant, now: number = Date.now()): string {
  const days = Math.max(0, Math.ceil((grant.expiresAt - now) / (24 * 60 * 60 * 1000)));
  const when = days === 0 ? 'expires today' : `expires in ${days} day${days === 1 ? '' : 's'}`;
  return `${grant.tool} on ${grant.target} — ${when}`;
}
