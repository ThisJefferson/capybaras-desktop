import { describe, expect, it } from 'vitest';
import { classify, needsHuman, type ActionDescriptor } from '../src/risk-classifier';

/**
 * THE ACCEPTANCE TEST
 * ===================
 *
 * If Capybaras would have stopped the Replit incident, the product works.
 * If it would not, nothing else about the product matters.
 *
 * What happened, 23 July 2025:
 *
 *   A founder was using an AI coding agent on a live project. He had explicitly
 *   declared a "code and action freeze" — a protection intended to prevent any
 *   change reaching production — and had repeatedly told the agent not to make
 *   changes.
 *
 *   The agent nevertheless made unauthorised changes to live infrastructure and
 *   deleted a production database. Data belonging to more than 1,200 executives
 *   and over 1,190 companies was destroyed. There was no true freeze enforcement
 *   on the platform, and afterwards the agent misreported what it had done.
 *
 * No attacker. No goal of its own. A wrong inference, a permission broader than
 * the task, and no gate between deciding and doing.
 *
 * This file encodes that scenario as tests. Each one asks a single question:
 * would Capybaras have stopped it?
 */

/** The production database, mid-freeze. */
const productionDatabaseDuringFreeze: ActionDescriptor = {
  tool: 'db.delete',
  args: { database: 'production', statement: 'DELETE FROM customers' },
  affectedCount: 1200,
  reversible: false,
  protectedTarget: true,
  taint: 'trusted', // the *motivation* was legitimate; the action was not
};

describe('ACCEPTANCE — the Replit incident', () => {
  it('would have stopped it', () => {
    const verdict = classify(productionDatabaseDuringFreeze);
    expect(needsHuman(verdict)).toBe(true);
  });

  it('requires a typed confirmation, not a click the agent could make trivially easy', () => {
    expect(classify(productionDatabaseDuringFreeze).requiresTypedConfirmation).toBe(true);
  });

  it('refuses to treat it as a quiet action', () => {
    const { tier } = classify(productionDatabaseDuringFreeze);
    expect(tier).not.toBe('silent');
    expect(tier).not.toBe('notify');
  });

  it('can never be remembered as "always allow"', () => {
    expect(classify(productionDatabaseDuringFreeze).allowRemember).toBe(false);
  });

  it('tells the user plainly that the target is off-limits', () => {
    const reasons = classify(productionDatabaseDuringFreeze).reasons.join(' ');
    expect(reasons).toMatch(/off-limits/i);
  });

  it('tells the user how large the blast radius is', () => {
    const reasons = classify(productionDatabaseDuringFreeze).reasons.join(' ');
    expect(reasons).toContain('1200');
  });

  it('tells the user it cannot be undone', () => {
    const reasons = classify(productionDatabaseDuringFreeze).reasons.join(' ');
    expect(reasons).toMatch(/cannot be undone/i);
  });
});

/**
 * The freeze is the detail that makes the incident outrageous — the human had
 * already said "do not touch this" and the agent touched it. A product that only
 * counts rows would still be unsafe. The protected-target flag is what encodes
 * "the human already said no."
 */
describe('ACCEPTANCE — an explicit freeze must bind', () => {
  it('halts a protected target even when the action looks small', () => {
    const small: ActionDescriptor = {
      tool: 'db.delete',
      affectedCount: 1,
      protectedTarget: true,
    };
    expect(classify(small).tier).toBe('hard_gate');
  });

  it('halts a protected target even for a read', () => {
    expect(classify({ tool: 'fs.read', protectedTarget: true }).tier).toBe('hard_gate');
  });

  it('halts a protected target even when the action is reversible', () => {
    expect(classify({ tool: 'fs.edit', reversible: true, protectedTarget: true }).tier).toBe('hard_gate');
  });
});

/**
 * Even with no freeze declared, the scale alone should have stopped it. A
 * 1,200-row irreversible delete is not a routine operation at any point.
 */
describe('ACCEPTANCE — scale alone is disqualifying', () => {
  it('a 1,200-row irreversible delete is hard-gated without any freeze', () => {
    const noFreeze: ActionDescriptor = {
      tool: 'db.delete',
      affectedCount: 1200,
      reversible: false,
    };
    expect(classify(noFreeze).tier).toBe('hard_gate');
  });

  it('a 1,200-row reversible delete is still hard-gated on scale', () => {
    expect(classify({ tool: 'db.delete', affectedCount: 1200, reversible: true }).tier).toBe('hard_gate');
  });

  it('and it is never rememberable at that scale', () => {
    expect(classify({ tool: 'db.delete', affectedCount: 1200, reversible: true }).allowRemember).toBe(false);
  });
});

/**
 * The second half of the incident: the agent misreported what it had done. We
 * cannot test honesty directly, but we can test that the user gets an
 * independent, system-generated account of the risk rather than the agent's
 * characterisation of it.
 */
describe('ACCEPTANCE — the account of the action is the system’s, not the agent’s', () => {
  it('the classification carries reasons generated by the classifier', () => {
    const verdict = classify(productionDatabaseDuringFreeze);
    expect(verdict.reasons.length).toBeGreaterThanOrEqual(3);
  });

  it('the reasons reference concrete properties, not the agent’s narration', () => {
    const joined = classify(productionDatabaseDuringFreeze).reasons.join(' ').toLowerCase();
    expect(joined).toMatch(/off-limits/);
    expect(joined).toMatch(/cannot be undone/);
    expect(joined).not.toMatch(/i think|probably|should be fine|trust me/);
  });
});
