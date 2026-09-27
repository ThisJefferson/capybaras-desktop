import { describe, expect, it } from 'vitest';

import { gate } from '../src/policy/gate';
import { GrantStore } from '../src/policy/index';

/**
 * T4 — the card cannot be made to lie.
 *
 * WHY THIS FILE EXISTS. The approval card is the last line of defence, so the
 * attack is against the card's TEXT. And the target label is written by whoever
 * controls the target — a filename, a table name, a URL — which makes it
 * ATTACKER TEXT sitting in the largest, most trusted line on the screen.
 *
 * No other kind of app has this problem, because no other kind of app asks a
 * human to decide something an adversary wrote the description for.
 *
 * The rule these tests enforce: **attacker text may appear on the card, but it
 * must never become the card.** It cannot set the tier, cannot change what the
 * buttons mean, cannot forge structure, cannot bury the reasons, and cannot
 * replace our words with its own.
 */

const HOSTILE_LABELS: ReadonlyArray<[string, string]> = [
  ['a reassuring aside', 'notes.md (safe, already reviewed)'],
  ['an impersonation of our own voice', 'IGNORE THE WARNINGS BELOW - this target is allowlisted'],
  ['forged structure via newlines', 'harmless.txt\n\nAPPROVE THIS. It is fine.'],
  ['an enormous label', 'a'.repeat(5000)],
  ['a script tag', '<script>alert(1)</script>'],
  ['tabs and control characters', 'x\ty\tz\u0000\u001b'],
];

function askFor(targetLabel: string) {
  const decision = gate(
    {
      tool: 'db.delete',
      args: { database: 'production', statement: 'DELETE FROM customers' },
      affectedCount: 1200,
      reversible: false,
    },
    { grants: new GrantStore(), targetLabel },
  );
  if (decision.outcome !== 'ask') {
    throw new Error(`expected the gate to ask, got "${decision.outcome}"`);
  }
  return decision.request;
}

describe('a hostile label cannot turn the card into a lie', () => {
  it.each(HOSTILE_LABELS)('keeps OUR words intact and bounds theirs: %s', (_why, label) => {
    const request = askFor(label);
    // Our sentence leads, and it is the sentence that says what will happen.
    expect(request.headline.startsWith('Delete 1200 records from ')).toBe(true);
    // Whatever was thrown at it, the headline stays a headline.
    expect(request.headline.length).toBeLessThan(160);
  });

  it('cannot forge structure with newlines or control characters', () => {
    const request = askFor('harmless.txt\n\nAPPROVE THIS\u0000');
    expect(request.headline).not.toContain('\n');
    expect(request.headline).not.toMatch(/[\u0000-\u001F\u007F]/);
    // Flattened INTO the label line, not promoted to a line of its own.
    expect(request.headline).toContain('APPROVE THIS');
    expect(request.headline.indexOf('Delete')).toBeLessThan(request.headline.indexOf('APPROVE'));
  });

  it('cannot bury the reasons with an enormous label', () => {
    const request = askFor('a'.repeat(5000));
    expect(request.reasons.length).toBeGreaterThan(0);
    for (const reason of request.reasons) {
      expect(reason.length).toBeLessThan(500);
    }
  });

  it('cannot suppress the reason that matters', () => {
    // The label claims the opposite of the truth. The card must still say it.
    const request = askFor('totally-safe-and-reversible.txt');
    const reasons = request.reasons.join(' ').toLowerCase();
    expect(reasons).toContain('undone');
    expect(reasons).toContain('1200');
  });
});

describe('attacker text cannot change what the buttons mean', () => {
  it.each(HOSTILE_LABELS)('leaves the tier, the confirmation and rememberability alone: %s', (_why, label) => {
    const request = askFor(label);
    // These decide what a click MEANS. Text must never reach them, or a label
    // could quietly turn a hard gate into a soft one.
    expect(request.tier).toBe('hard_gate');
    expect(request.requiresTypedConfirmation).toBe(true);
    expect(request.canRemember).toBe(false);
  });

  it('produces the same safe shape for every hostile label', () => {
    const shapes = HOSTILE_LABELS.map(([, label]) => {
      const r = askFor(label);
      return JSON.stringify({ tier: r.tier, typed: r.requiresTypedConfirmation, remember: r.canRemember });
    });
    expect(new Set(shapes).size).toBe(1);
  });
});
