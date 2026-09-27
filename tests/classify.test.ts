import { describe, expect, it } from 'vitest';
import { classify, type ActionDescriptor, type RiskTier } from '../src/risk-classifier';

/**
 * The case corpus.
 *
 * Every test here is a claim about how much we interrupt a human. The corpus is
 * deliberately opinionated: if you disagree with a case, the argument is about
 * policy, not about a bug. Change the case and the code together, and say why
 * in DECISIONS.md.
 */

const tierOf = (a: ActionDescriptor): RiskTier => classify(a).tier;

/* ------------------------------------------------------------------ */
describe('baseline: reading is free', () => {
  it.each([
    ['fs.read'],
    ['fs.list'],
    ['fs.stat'],
    ['web.fetch'],
    ['web.search'],
    ['clock.read'],
  ])('%s runs silently', (tool) => {
    expect(tierOf({ tool })).toBe('silent');
  });
});

/* ------------------------------------------------------------------ */
describe('baseline: creating is cheap to undo', () => {
  it.each([['fs.create'], ['fs.mkdir']])('%s notifies only', (tool) => {
    expect(tierOf({ tool })).toBe('notify');
  });
});

/* ------------------------------------------------------------------ */
describe('baseline: changing existing things asks first', () => {
  it.each([
    ['fs.write'],
    ['fs.edit'],
    ['fs.move'],
    ['fs.rename'],
    ['fs.delete'],
    ['fs.truncate'],
    ['db.delete'],
    ['db.update'],
    ['exec'],
  ])('%s requires confirmation', (tool) => {
    expect(tierOf({ tool })).toBe('confirm');
  });
});

/* ------------------------------------------------------------------ */
describe('baseline: sending and spending need consent', () => {
  it.each([['message.send'], ['mail.send'], ['net.post'], ['net.put'], ['net.delete']])(
    '%s requires confirmation',
    (tool) => {
      expect(tierOf({ tool })).toBe('confirm');
    },
  );

  it.each([['spend'], ['payment.send'], ['credentials.read'], ['credentials.write'], ['config.security'], ['agent.selfModify']])(
    '%s is hard-gated',
    (tool) => {
      expect(tierOf({ tool })).toBe('hard_gate');
    },
  );
});

/* ------------------------------------------------------------------ */
describe('unknown tools fail safe', () => {
  it('an unrecognised tool needs consent rather than running free', () => {
    expect(tierOf({ tool: 'totally.unknown' })).toBe('confirm');
  });

  it('and it explains itself', () => {
    const c = classify({ tool: 'totally.unknown' });
    expect(c.reasons.join(' ')).toMatch(/unrecognised/i);
  });
});

/* ------------------------------------------------------------------ */
describe('blast radius', () => {
  it('a handful of objects is still just a confirmation', () => {
    expect(tierOf({ tool: 'fs.delete', affectedCount: 3 })).toBe('confirm');
  });

  it('10 objects is the boundary and stays a confirmation', () => {
    expect(tierOf({ tool: 'fs.delete', affectedCount: 10 })).toBe('confirm');
  });

  it('11 objects escalates', () => {
    expect(tierOf({ tool: 'fs.delete', affectedCount: 11 })).toBe('hard_gate');
  });

  it('1500 objects is catastrophic', () => {
    expect(tierOf({ tool: 'fs.delete', affectedCount: 1500 })).toBe('hard_gate');
  });

  it('the count appears in the explanation', () => {
    const c = classify({ tool: 'fs.delete', affectedCount: 1200 });
    expect(c.reasons.join(' ')).toContain('1200');
  });

  it('a mass read is escalated out of silent', () => {
    expect(tierOf({ tool: 'web.fetch', affectedCount: 500 })).toBe('hard_gate');
  });
});

/* ------------------------------------------------------------------ */
describe('reversibility', () => {
  it('an irreversible edit is at least a confirmation', () => {
    expect(tierOf({ tool: 'fs.edit', reversible: false })).toBe('confirm');
  });

  it('an irreversible create cannot stay at notify', () => {
    expect(tierOf({ tool: 'fs.create', reversible: false })).toBe('confirm');
  });

  it('irreversible plus many objects is hard-gated', () => {
    expect(tierOf({ tool: 'fs.edit', reversible: false, affectedCount: 50 })).toBe('hard_gate');
  });

  it('says plainly that it cannot be undone', () => {
    const c = classify({ tool: 'fs.edit', reversible: false });
    expect(c.reasons.join(' ')).toMatch(/cannot be undone/i);
  });

  it('a reversible action gains no such warning', () => {
    const c = classify({ tool: 'fs.edit', reversible: true });
    expect(c.reasons.join(' ')).not.toMatch(/cannot be undone/i);
  });
});

/* ------------------------------------------------------------------ */
describe('leaving the machine', () => {
  it('publishing escalates a silent read', () => {
    expect(tierOf({ tool: 'fs.read', leavesMachine: true })).toBe('confirm');
  });

  it('an external message is at least a confirmation', () => {
    expect(tierOf({ tool: 'message.send', leavesMachine: true })).toBe('confirm');
  });

  it('explains that it reaches outside', () => {
    const c = classify({ tool: 'net.post', leavesMachine: true });
    expect(c.reasons.join(' ')).toMatch(/outside your computer/i);
  });
});

/* ------------------------------------------------------------------ */
describe('motivation taint — where the instruction came from', () => {
  it('a trusted read stays silent', () => {
    expect(tierOf({ tool: 'fs.read', taint: 'trusted' })).toBe('silent');
  });

  it('a read motivated by untrusted content is escalated', () => {
    expect(tierOf({ tool: 'fs.read', taint: 'untrusted' })).toBe('confirm');
  });

  it('mixed provenance is treated as untrusted for escalation', () => {
    expect(tierOf({ tool: 'fs.read', taint: 'mixed' })).toBe('confirm');
  });

  it('an untrusted delete is hard-gated', () => {
    expect(tierOf({ tool: 'fs.delete', taint: 'untrusted' })).toBe('hard_gate');
  });

  it('an untrusted shell command is hard-gated', () => {
    expect(tierOf({ tool: 'exec', taint: 'untrusted' })).toBe('hard_gate');
  });

  it('an untrusted outward message is hard-gated', () => {
    expect(tierOf({ tool: 'net.post', taint: 'untrusted' })).toBe('hard_gate');
  });

  it('the reason names the untrusted source', () => {
    const c = classify({ tool: 'fs.edit', taint: 'untrusted' });
    expect(c.reasons.join(' ')).toMatch(/content the agent read/i);
  });

  it('an untrusted harmless action still gets a confirmation', () => {
    expect(tierOf({ tool: 'web.search', taint: 'untrusted' })).toBe('confirm');
  });
});

/* ------------------------------------------------------------------ */
describe('destructive argument patterns', () => {
  it.each([
    ['recursive force delete', 'rm -rf /'],
    ['windows force delete', 'del /f /q C:\\important'],
    ['windows recursive rmdir', 'rmdir /s /q C:\\stuff'],
    ['database drop', 'DROP TABLE customers'],
    ['database truncate', 'truncate table orders'],
    ['disk format', 'mkfs.ext4 /dev/sdb1'],
    ['destructive git reset', 'git reset --hard HEAD~50'],
    ['destructive git clean', 'git clean -fd'],
  ])('detects %s', (_label, command) => {
    expect(tierOf({ tool: 'exec', args: { command } })).toBe('hard_gate');
  });

  it('records which pattern matched', () => {
    const c = classify({ tool: 'exec', args: { command: 'rm -rf /var' } });
    expect(c.blockedPatterns.length).toBeGreaterThan(0);
  });

  it('a benign shell command stays at confirmation', () => {
    expect(tierOf({ tool: 'exec', args: { command: 'ls -la' } })).toBe('confirm');
  });

  it('finds destructive strings nested in arguments', () => {
    expect(tierOf({ tool: 'exec', args: { opts: { script: ['echo hi', 'rm -rf /'] } } })).toBe('hard_gate');
  });
});

/* ------------------------------------------------------------------ */
describe('protected targets', () => {
  it('a protected target is hard-gated even for a harmless action', () => {
    expect(tierOf({ tool: 'fs.read', protectedTarget: true })).toBe('hard_gate');
  });

  it('says the target is off-limits', () => {
    const c = classify({ tool: 'fs.read', protectedTarget: true });
    expect(c.reasons.join(' ')).toMatch(/off-limits/i);
  });

  it('and it can never be remembered', () => {
    expect(classify({ tool: 'fs.edit', protectedTarget: true }).allowRemember).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('secrets, money, and security settings', () => {
  it('touching secrets is hard-gated', () => {
    expect(tierOf({ tool: 'fs.read', touchesSecrets: true })).toBe('hard_gate');
  });

  it('spending is hard-gated', () => {
    expect(tierOf({ tool: 'fs.edit', involvesMoney: true })).toBe('hard_gate');
  });

  it('changing security settings is hard-gated', () => {
    expect(tierOf({ tool: 'fs.edit', changesSecurityConfig: true })).toBe('hard_gate');
  });
});

/* ------------------------------------------------------------------ */
describe('what the interface may offer to remember', () => {
  it('a plain confirmation may be remembered', () => {
    expect(classify({ tool: 'fs.edit' }).allowRemember).toBe(true);
  });

  it('a hard gate is never rememberable', () => {
    expect(classify({ tool: 'credentials.read' }).allowRemember).toBe(false);
  });

  it('an untrusted-motivated action is never rememberable', () => {
    expect(classify({ tool: 'fs.edit', taint: 'untrusted' }).allowRemember).toBe(false);
  });

  it('an irreversible action is never rememberable', () => {
    expect(classify({ tool: 'fs.edit', reversible: false }).allowRemember).toBe(false);
  });

  it('a silent action has nothing to remember', () => {
    expect(classify({ tool: 'fs.read' }).allowRemember).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('typed confirmation', () => {
  it('hard gates require typing, not clicking', () => {
    expect(classify({ tool: 'spend' }).requiresTypedConfirmation).toBe(true);
  });

  it('ordinary confirmations do not', () => {
    expect(classify({ tool: 'fs.edit' }).requiresTypedConfirmation).toBe(false);
  });

  it('silent actions do not', () => {
    expect(classify({ tool: 'fs.read' }).requiresTypedConfirmation).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('property: escalation only', () => {
  const baseline: Record<string, RiskTier> = {
    'fs.read': 'silent',
    'fs.create': 'notify',
    'fs.delete': 'confirm',
    'exec': 'confirm',
    'credentials.read': 'hard_gate',
  };

  it.each(Object.entries(baseline))('%s can never be classified below its baseline', (tool, base) => {
    const variations: ActionDescriptor[] = [
      { tool },
      { tool, affectedCount: 1 },
      { tool, reversible: true },
      { tool, taint: 'trusted' },
      { tool, leavesMachine: false },
    ];
    for (const v of variations) {
      const order: Record<RiskTier, number> = { silent: 0, notify: 1, confirm: 2, hard_gate: 3 };
      expect(order[classify(v).tier]).toBeGreaterThanOrEqual(order[base]);
    }
  });

  it('adding a hazard never lowers the tier', () => {
    const order: Record<RiskTier, number> = { silent: 0, notify: 1, confirm: 2, hard_gate: 3 };
    const plain = classify({ tool: 'fs.edit' }).tier;
    const hazards: ActionDescriptor[] = [
      { tool: 'fs.edit', reversible: false },
      { tool: 'fs.edit', affectedCount: 50 },
      { tool: 'fs.edit', taint: 'untrusted' },
      { tool: 'fs.edit', touchesSecrets: true },
      { tool: 'fs.edit', leavesMachine: true },
      { tool: 'fs.edit', protectedTarget: true },
    ];
    for (const h of hazards) {
      expect(order[classify(h).tier]).toBeGreaterThanOrEqual(order[plain]);
    }
  });

  it('every escalation carries a reason', () => {
    const escalated: ActionDescriptor[] = [
      { tool: 'fs.delete', affectedCount: 500 },
      { tool: 'fs.delete', taint: 'untrusted' },
      { tool: 'fs.read', touchesSecrets: true },
      { tool: 'fs.read', leavesMachine: true },
    ];
    for (const a of escalated) {
      expect(classify(a).reasons.length).toBeGreaterThan(0);
    }
  });
});
