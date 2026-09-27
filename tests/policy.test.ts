import { describe, expect, it } from 'vitest';
import { GrantStore, describeGrant, DEFAULT_TTL_MS } from '../src/policy';
import { gate } from '../src/policy';
import { classify, type ActionDescriptor } from '../src/risk-classifier';

const DAY = 24 * 60 * 60 * 1000;
const T0 = 1_800_000_000_000; // fixed clock so tests never drift

const liveClassification = classify({ tool: 'fs.edit' });

/* ================================================================== */
describe('grants — only a human, only one target, and never a hard gate', () => {
  it('records an approval for a plain confirmation', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: liveClassification,
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(true);
  });

  it('refuses anything not approved by a person', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: liveClassification,
      // deliberately wrong: the model must have no path here
      approvedBy: 'model' as unknown as 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
  });

  it('refuses to remember a hard gate', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'spend', target: 'the shop' },
      classification: classify({ tool: 'spend' }),
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toMatch(/never be remembered/i);
  });

  it('refuses to remember a silent read', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.read', target: 'Documents/notes.md' },
      classification: classify({ tool: 'fs.read' }),
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
  });

  it('refuses to remember an untrusted-motivated action', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: classify({ tool: 'fs.edit', taint: 'untrusted' }),
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
  });

  it('refuses to remember an irreversible action', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: classify({ tool: 'fs.edit', reversible: false }),
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
  });

  it('refuses a wildcard target', () => {
    const store = new GrantStore();
    for (const target of ['*', 'Documents/*', 'all', 'any', 'everything']) {
      const result = store.record({
        scope: { tool: 'fs.edit', target },
        classification: liveClassification,
        approvedBy: 'human',
        now: T0,
      });
      expect(result.ok, `should refuse target "${target}"`).toBe(false);
    }
  });

  it('refuses an empty target', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: '' },
      classification: liveClassification,
      approvedBy: 'human',
      now: T0,
    });
    expect(result.ok).toBe(false);
  });

  it('refuses a grant that would never expire', () => {
    const store = new GrantStore();
    const result = store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: liveClassification,
      approvedBy: 'human',
      now: T0,
      ttlMs: 0,
    });
    expect(result.ok).toBe(false);
  });
});

/* ================================================================== */
describe('grants — lookup, expiry, revocation', () => {
  const seed = (now = T0) => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: liveClassification,
      approvedBy: 'human',
      now,
    });
    return store;
  };

  it('finds a live grant by exact scope', () => {
    expect(seed().find({ tool: 'fs.edit', target: 'Documents/notes.md' }, T0)).toBeDefined();
  });

  it('does not match a different target', () => {
    expect(seed().find({ tool: 'fs.edit', target: 'Documents/other.md' }, T0)).toBeUndefined();
  });

  it('does not match a different tool', () => {
    expect(seed().find({ tool: 'fs.write', target: 'Documents/notes.md' }, T0)).toBeUndefined();
  });

  it('expires after the default window', () => {
    const store = seed();
    const later = T0 + DEFAULT_TTL_MS + 1;
    expect(store.find({ tool: 'fs.edit', target: 'Documents/notes.md' }, later)).toBeUndefined();
  });

  it('is still live one second before expiry', () => {
    const store = seed();
    const almost = T0 + DEFAULT_TTL_MS - 1000;
    expect(store.find({ tool: 'fs.edit', target: 'Documents/notes.md' }, almost)).toBeDefined();
  });

  it('honours a custom ttl', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents/notes.md' },
      classification: liveClassification,
      approvedBy: 'human',
      now: T0,
      ttlMs: DAY,
    });
    expect(store.find({ tool: 'fs.edit', target: 'Documents/notes.md' }, T0 + DAY + 1)).toBeUndefined();
  });

  it('lists live grants soonest-expiring first', () => {
    const store = new GrantStore();
    store.record({ scope: { tool: 'fs.edit', target: 'a' }, classification: liveClassification, approvedBy: 'human', now: T0, ttlMs: 10 * DAY });
    store.record({ scope: { tool: 'fs.edit', target: 'b' }, classification: liveClassification, approvedBy: 'human', now: T0, ttlMs: 1 * DAY });
    const listed = store.list(T0);
    expect(listed[0]?.target).toBe('b');
  });

  it('revokes one', () => {
    const store = seed();
    const [grant] = store.list(T0);
    expect(grant).toBeDefined();
    expect(store.revoke(grant!.id)).toBe(true);
    expect(store.list(T0)).toHaveLength(0);
  });

  it('revokes everything', () => {
    const store = seed();
    store.record({ scope: { tool: 'fs.edit', target: 'x' }, classification: liveClassification, approvedBy: 'human', now: T0 });
    expect(store.revokeAll()).toBe(2);
    expect(store.list(T0)).toHaveLength(0);
  });

  it('prunes only the expired ones', () => {
    const store = new GrantStore();
    store.record({ scope: { tool: 'fs.edit', target: 'a' }, classification: liveClassification, approvedBy: 'human', now: T0, ttlMs: DAY });
    store.record({ scope: { tool: 'fs.edit', target: 'b' }, classification: liveClassification, approvedBy: 'human', now: T0, ttlMs: 90 * DAY });
    expect(store.prune(T0 + 2 * DAY)).toBe(1);
    expect(store.list(T0 + 2 * DAY)).toHaveLength(1);
  });

  it('describes a grant in plain language', () => {
    const store = seed();
    const text = describeGrant(store.list(T0)[0]!, T0);
    expect(text).toMatch(/expires in 30 days/);
    expect(text).toContain('Documents/notes.md');
  });
});

/* ================================================================== */
describe('gate — proceed', () => {
  const fresh = () => new GrantStore();

  it('lets a read through, silently', () => {
    const decision = gate({ tool: 'fs.read' }, { grants: fresh(), now: T0 });
    expect(decision.outcome).toBe('proceed');
  });

  it('lets a create through, with a note afterwards', () => {
    const decision = gate({ tool: 'fs.create' }, { grants: fresh(), now: T0 });
    expect(decision.outcome).toBe('proceed');
    if (decision.outcome === 'proceed') expect(decision.because).toMatch(/afterwards/i);
  });
});

/* ================================================================== */
describe('gate — asking', () => {
  const fresh = () => new GrantStore();

  it('stops a delete and produces a request', () => {
    const decision = gate({ tool: 'fs.delete' }, { grants: fresh(), now: T0, targetLabel: 'Documents' });
    expect(decision.outcome).toBe('ask');
  });

  it('the request is one plain sentence, never a tool id', () => {
    const decision = gate({ tool: 'fs.delete', affectedCount: 3 }, { grants: fresh(), now: T0, targetLabel: 'Documents' });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.headline).toBe('Delete 3 files in Documents?');
    expect(decision.request.headline).not.toContain('fs.delete');
  });

  it('an unrecognised tool still asks, in readable language', () => {
    const decision = gate({ tool: 'mystery.action' }, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.headline).toContain('mystery.action');
  });

  it('carries every reason, not just the first', () => {
    const action: ActionDescriptor = {
      tool: 'db.delete',
      affectedCount: 1200,
      reversible: false,
      protectedTarget: true,
    };
    const decision = gate(action, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.reasons.length).toBeGreaterThanOrEqual(3);
  });
});

/* ================================================================== */
describe('gate — a remembered choice satisfies a confirm, and nothing else', () => {
  it('a matching grant lets the same action through', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate({ tool: 'fs.edit' }, { grants: store, now: T0, targetLabel: 'Documents' });
    expect(decision.outcome).toBe('proceed');
    if (decision.outcome === 'proceed') expect(decision.because).toMatch(/before/i);
  });

  it('a grant for a different target does not carry over', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate({ tool: 'fs.edit' }, { grants: store, now: T0, targetLabel: 'Photos' });
    expect(decision.outcome).toBe('ask');
  });

  it('an expired grant does not carry over', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate({ tool: 'fs.edit' }, { grants: store, now: T0 + 400 * DAY, targetLabel: 'Documents' });
    expect(decision.outcome).toBe('ask');
  });

  it('A HARD GATE IS NEVER SATISFIED BY A GRANT', () => {
    const store = new GrantStore();
    // Force a grant in for the same scope by using a rememberable action,
    // then gate a hard-gated variant of it.
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate(
      { tool: 'fs.edit', touchesSecrets: true },
      { grants: store, now: T0, targetLabel: 'Documents' },
    );
    expect(decision.outcome).toBe('ask');
    if (decision.outcome === 'ask') expect(decision.request.tier).toBe('hard_gate');
  });

  it('an untrusted-motivated action is never satisfied by a grant', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate(
      { tool: 'fs.edit', taint: 'untrusted' },
      { grants: store, now: T0, targetLabel: 'Documents' },
    );
    expect(decision.outcome).toBe('ask');
  });
});

/* ================================================================== */
describe('gate — hard gates ask for typing', () => {
  const fresh = () => new GrantStore();

  it('marks a hard gate as requiring typed confirmation', () => {
    const decision = gate({ tool: 'spend' }, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.tier).toBe('hard_gate');
    expect(decision.request.requiresTypedConfirmation).toBe(true);
  });

  it('supplies the phrase the human must type', () => {
    const decision = gate({ tool: 'spend' }, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.confirmationPhrase).toBeTruthy();
    expect(decision.request.confirmationPhrase).not.toMatch(/\?$/);
  });

  it('never offers to remember a hard gate', () => {
    const decision = gate({ tool: 'credentials.read' }, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.canRemember).toBe(false);
  });

  it('an ordinary confirmation offers no typing, but may be remembered', () => {
    const decision = gate({ tool: 'fs.delete' }, { grants: fresh(), now: T0 });
    if (decision.outcome !== 'ask') throw new Error('expected ask');
    expect(decision.request.requiresTypedConfirmation).toBe(false);
    expect(decision.request.canRemember).toBe(true);
    expect(decision.request.confirmationPhrase).toBeUndefined();
  });
});

/* ================================================================== */
describe('gate — dry run can only add caution', () => {
  const fresh = () => new GrantStore();

  it('shows the plan for anything above silent', () => {
    const decision = gate({ tool: 'fs.delete' }, { grants: fresh(), now: T0, dryRun: true });
    expect(decision.outcome).toBe('dry_run');
  });

  it('the plan carries the headline and the reasons', () => {
    const decision = gate(
      { tool: 'fs.delete', affectedCount: 12 },
      { grants: fresh(), now: T0, dryRun: true, targetLabel: 'Downloads' },
    );
    if (decision.outcome !== 'dry_run') throw new Error('expected dry_run');
    expect(decision.headline).toContain('Downloads');
    expect(decision.reasons.length).toBeGreaterThan(0);
  });

  it('leaves a silent read alone', () => {
    const decision = gate({ tool: 'fs.read' }, { grants: fresh(), now: T0, dryRun: true });
    expect(decision.outcome).toBe('proceed');
  });

  it('does NOT turn an ask into a proceed', () => {
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.edit', target: 'Documents' },
      classification: classify({ tool: 'fs.edit' }),
      approvedBy: 'human',
      now: T0,
    });
    const decision = gate({ tool: 'fs.edit' }, { grants: store, now: T0, dryRun: true, targetLabel: 'Documents' });
    expect(decision.outcome).toBe('dry_run');
  });

  it('never executes: dry run returns no proceed for a hard gate', () => {
    const decision = gate({ tool: 'spend' }, { grants: fresh(), now: T0, dryRun: true });
    expect(decision.outcome).not.toBe('proceed');
  });
});

/* ================================================================== */
describe('ACCEPTANCE through the gate — the Replit scenario', () => {
  it('is stopped, and the request says everything a human needs', () => {
    const decision = gate(
      {
        tool: 'db.delete',
        args: { database: 'production' },
        affectedCount: 1200,
        reversible: false,
        protectedTarget: true,
      },
      { grants: new GrantStore(), now: T0, targetLabel: 'the production database' },
    );

    expect(decision.outcome).toBe('ask');
    if (decision.outcome !== 'ask') throw new Error('expected ask');

    expect(decision.request.tier).toBe('hard_gate');
    expect(decision.request.requiresTypedConfirmation).toBe(true);
    expect(decision.request.canRemember).toBe(false);
    expect(decision.request.headline).toContain('1200');
    expect(decision.request.reasons.join(' ')).toMatch(/off-limits/i);
    expect(decision.request.reasons.join(' ')).toMatch(/cannot be undone/i);
  });

  it('is stopped even in dry-run mode', () => {
    const decision = gate(
      { tool: 'db.delete', affectedCount: 1200, protectedTarget: true },
      { grants: new GrantStore(), now: T0, dryRun: true },
    );
    expect(decision.outcome).toBe('dry_run');
    expect(decision.outcome).not.toBe('proceed');
  });
});
