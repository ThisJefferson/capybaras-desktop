import { describe, expect, it } from 'vitest';

import { gate } from '../src/policy/gate';
import type { ProtectionPolicy } from '../src/policy/protection';
import { loadProtectionPolicy, resolveProtection } from '../src/policy/protection';
import { GrantStore } from '../src/policy/index';

const readFrom = (contents: string | undefined) => () => {
  if (contents === undefined) {
    const error = new Error('no such file') as NodeJS.ErrnoException;
    error.code = 'ENOENT';
    throw error;
  }
  return contents;
};

describe('the invariant that matters', () => {
  it('lets an action ADD caution', () => {
    const result = resolveProtection({ tool: 'fs.write' }, {}, { claimedProtected: true });
    expect(result.protectedTarget).toBe(true);
  });

  /**
   * THE WHOLE POINT. Before this change `protectedTarget` came from the action
   * descriptor -- from the caller. An agent that wanted to avoid a freeze simply
   * did not declare one. So a policy match must survive an action that claims
   * the opposite.
   */
  it('does NOT let an action remove caution the policy imposes', () => {
    const policy: ProtectionPolicy = { protectedTargets: ['*production*'] };
    const result = resolveProtection(
      { tool: 'db.delete', args: { database: 'production' } },
      policy,
      { claimedProtected: false },
    );
    expect(result.protectedTarget).toBe(true);
    expect(result.reasons.join(' ')).toMatch(/off-limits/i);
  });

  it('catches a protected target named in the label rather than the arguments', () => {
    const policy: ProtectionPolicy = { protectedTargets: ['*production*'] };
    const result = resolveProtection({ tool: 'db.delete' }, policy, {
      targetLabel: 'the production database',
    });
    expect(result.protectedTarget).toBe(true);
  });

  it('catches a protected tool whatever the arguments say', () => {
    const policy: ProtectionPolicy = { protectedTools: ['credentials.read'] };
    expect(resolveProtection({ tool: 'credentials.read' }, policy).protectedTarget).toBe(true);
    expect(resolveProtection({ tool: 'fs.read' }, policy).protectedTarget).toBe(false);
  });

  it('does not match an unrelated target', () => {
    const policy: ProtectionPolicy = { protectedTargets: ['*production*'] };
    const result = resolveProtection({ tool: 'fs.write', args: { path: 'notes.md' } }, policy);
    expect(result.protectedTarget).toBe(false);
  });
});

describe('freezes', () => {
  it('reports an active freeze with its stated reason', () => {
    const policy: ProtectionPolicy = {
      freeze: { active: true, reason: 'declared change freeze', declaredBy: 'ops' },
    };
    const result = resolveProtection({ tool: 'fs.write' }, policy);
    expect(result.freezeActive).toBe(true);
    expect(result.reasons.join(' ')).toMatch(/change freeze/i);
  });

  it('treats an inactive freeze as no freeze', () => {
    expect(resolveProtection({ tool: 'fs.write' }, { freeze: { active: false } }).freezeActive).toBe(false);
  });
});

describe('the gate applies the policy', () => {
  it('is inert with no policy — existing behaviour is unchanged', () => {
    // This is what makes the change safe: every existing caller passes no
    // policy, so nothing about their classification moves.
    const decision = gate({ tool: 'fs.read', args: { path: 'notes.md' } }, { grants: new GrantStore() });
    expect(decision.outcome).toBe('proceed');
    expect(decision.classification.tier).toBe('silent');
  });

  it('raises a write to a confirm when a freeze is in force, and says so', () => {
    const withFreeze = gate(
      { tool: 'fs.write', args: { path: 'notes.md' } },
      { grants: new GrantStore(), policy: { freeze: { active: true, reason: 'code freeze' } } },
    );
    expect(withFreeze.outcome).toBe('ask');
    expect(withFreeze.classification.reasons.join(' ')).toMatch(/freeze/i);
  });

  it('lets a READ proceed during a freeze, because a freeze is about changes', () => {
    // Gating reads would be surprising and would not prevent the harm. A control
    // people route around is worse than none, because it also removes the signal.
    const decision = gate(
      { tool: 'fs.read', args: { path: 'notes.md' } },
      { grants: new GrantStore(), policy: { freeze: { active: true, reason: 'code freeze' } } },
    );
    expect(decision.classification.tier).toBe('silent');
    expect(decision.outcome).toBe('proceed');
  });

  it('hard-gates an action whose policy-protected target the action did not declare', () => {
    const decision = gate(
      { tool: 'db.delete', args: { database: 'production' }, affectedCount: 1 },
      { grants: new GrantStore(), policy: { protectedTargets: ['*production*'] } },
    );
    expect(decision.outcome).toBe('ask');
    expect(decision.classification.tier).toBe('hard_gate');
    expect(decision.classification.reasons.join(' ')).toMatch(/off-limits/i);
  });
});

describe('loading a policy FAILS CLOSED', () => {
  it('treats an absent file as a machine with no policy, and says nothing', () => {
    const loaded = loadProtectionPolicy(readFrom(undefined), 'policy.json');
    expect(loaded.problem).toBeUndefined();
    expect(loaded.policy).toEqual({});
  });

  it('treats unreadable JSON as a policy that protects EVERYTHING', () => {
    const loaded = loadProtectionPolicy(readFrom('{ not json'), 'policy.json');
    expect(loaded.problem).toMatch(/not valid JSON/i);
    // The critical part: a corrupt file must never silently DISABLE a freeze.
    expect(resolveProtection({ tool: 'fs.write' }, loaded.policy).freezeActive).toBe(true);
  });

  it('treats a malformed shape as a policy that protects everything', () => {
    const loaded = loadProtectionPolicy(readFrom('{"protectedTargets": "not-an-array"}'), 'policy.json');
    expect(loaded.problem).toMatch(/malformed/i);
    // The FREEZE is what protects here. A target pattern needs a target to match
    // against, and this action has none -- the freeze does not care.
    expect(resolveProtection({ tool: 'fs.write' }, loaded.policy).freezeActive).toBe(true);
  });

  it('accepts a well-formed policy', () => {
    const loaded = loadProtectionPolicy(
      readFrom(JSON.stringify({ freeze: { active: true }, protectedTargets: ['*secret*'] })),
      'policy.json',
    );
    expect(loaded.problem).toBeUndefined();
    expect(loaded.policy.freeze?.active).toBe(true);
  });
});
