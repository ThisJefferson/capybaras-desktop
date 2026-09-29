import { describe, expect, it } from 'vitest';

import { classify, needsHuman } from '../src/risk-classifier';
import { describeFileAction, inspect } from '../src/file-inspector';

/**
 * The file-action corpus.
 *
 * Every case is a claim about *how much we interrupt a person* before something
 * happens to a file. The assertions run through the real classifier, because the
 * thing worth testing is the hinge, not either side of it: a descriptor that
 * looks right and lands in the wrong tier is the failure that matters.
 *
 * `notes` is what the card says. It is asserted on, but loosely — the sentences
 * may be reworded. What may NOT change is that the destination is named (T14)
 * and that an overwrite says what it replaces (T15).
 */

const ascii = (s: string): number[] => Array.from(s, (c) => c.charCodeAt(0));
const bytes = (...parts: Array<string | number[] | Uint8Array>): Uint8Array => {
  const out: number[] = [];
  for (const p of parts) {
    if (typeof p === 'string') out.push(...ascii(p));
    else if (p instanceof Uint8Array) out.push(...Array.from(p));
    else out.push(...p);
  }
  return new Uint8Array(out);
};

const PDF = '%PDF-1.7\n';
const benign = inspect('notes.pdf', bytes(PDF));
const scripted = inspect('notes.pdf', bytes(PDF, '/JavaScript (app.alert(1))'));
const macroDoc = inspect('sheet.xls', bytes(
  new Uint8Array([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]),
  '_VBA_PROJECT',
));

/** The tier the gate would reach for a given file action. */
const tierFor = (input: Parameters<typeof describeFileAction>[0]) =>
  classify(describeFileAction(input).descriptor).tier;

/* ------------------------------------------------------------------ */
describe('rule 1 — reading is free, and what you read is untrusted (T12)', () => {
  it('does not interrupt a read', () => {
    expect(tierFor({ action: 'read', path: '/home/me/a.pdf' })).toBe('silent');
    expect(tierFor({ action: 'list', path: '/home/me' })).toBe('silent');
  });

  it('does not escalate the read itself — the person asked for it', () => {
    const { descriptor } = describeFileAction({ action: 'read', path: '/home/me/a.pdf' });
    expect(descriptor.taint).toBe('trusted');
  });

  it('marks what comes OUT as untrusted, so it cannot authorise anything', () => {
    const { resultTaint } = describeFileAction({ action: 'read', path: '/home/me/a.pdf' });
    expect(resultTaint).toBe('untrusted');
  });

  it('does not soften a taint that was already worse', () => {
    expect(
      describeFileAction({ action: 'read', path: '/n', taint: 'mixed' }).resultTaint,
    ).toBe('mixed');
    expect(
      describeFileAction({ action: 'read', path: '/n', taint: 'untrusted' }).resultTaint,
    ).toBe('untrusted');
  });

  it('says so when the file it is reading is not an ordinary one', () => {
    const { notes } = describeFileAction({ action: 'read', path: '/n/a.pdf', inspection: scripted });
    expect(notes.join(' ')).toMatch(/not treated as an instruction/i);
  });
});

/* ------------------------------------------------------------------ */
describe('rule 2 — every write is a confirm, and the card names the destination (T14)', () => {
  it('stops and asks, even for a brand new file', () => {
    expect(tierFor({ action: 'write', path: '/home/me/new.txt' })).toBe('confirm');
  });

  it('names the destination on the card', () => {
    const { notes } = describeFileAction({ action: 'write', path: '/home/me/report.pdf' });
    expect(notes.join(' ')).toContain('/home/me/report.pdf');
  });

  it('raises the stakes when the destination is outside the folder the person chose', () => {
    const { descriptor, notes } = describeFileAction({
      action: 'write',
      path: '//share/corp/out.csv',
      insideWorkspace: false,
    });
    expect(descriptor.leavesMachine).toBe(true);
    expect(notes.join(' ')).toMatch(/outside the folder/i);
    expect(needsHuman(classify(descriptor))).toBe(true);
  });
});

/* ------------------------------------------------------------------ */
describe('rule 3 — an existing target is a question, not an inconvenience (T15)', () => {
  it('marks an overwrite irreversible and says what would be lost', () => {
    const { descriptor, notes } = describeFileAction({
      action: 'write',
      path: '/home/me/thesis.md',
      targetExists: true,
    });
    expect(descriptor.reversible).toBe(false);
    expect(notes.join(' ')).toMatch(/already exists/i);
    expect(classify(descriptor).reasons.join(' ')).toMatch(/cannot be undone/i);
  });

  it('does not mark a new file irreversible', () => {
    const { descriptor } = describeFileAction({
      action: 'write',
      path: '/home/me/new.md',
      targetExists: false,
    });
    expect(descriptor.reversible).not.toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('rule 4 — opening is an action (T13)', () => {
  it('asks before opening anything', () => {
    expect(tierFor({ action: 'open', path: '/home/me/a.pdf' })).toBe('confirm');
  });

  it('tells the classifier when the file can act on its own', () => {
    const { descriptor } = describeFileAction({
      action: 'open',
      path: '/home/me/sheet.xls',
      inspection: macroDoc,
    });
    expect(descriptor.carriesExecutableContent).toBe(true);
    expect(classify(descriptor).reasons.join(' ')).toMatch(/act on its own/i);
  });

  it('does not mark an ordinary document as self-acting', () => {
    const { descriptor } = describeFileAction({
      action: 'open',
      path: '/home/me/notes.pdf',
      inspection: benign,
    });
    expect(descriptor.carriesExecutableContent).toBeUndefined();
  });

  it('leaves a trusted self-acting file at confirm — you can see whose file it is', () => {
    expect(
      tierFor({ action: 'open', path: '/home/me/sheet.xls', inspection: macroDoc }),
    ).toBe('confirm');
  });

  it('hard-gates when something you did not ask told you to open it', () => {
    const { descriptor } = describeFileAction({
      action: 'open',
      path: '/tmp/invoice.docm',
      inspection: macroDoc,
      taint: 'untrusted',
    });
    const c = classify(descriptor);
    expect(c.tier).toBe('hard_gate');
    expect(c.requiresTypedConfirmation).toBe(true);
    expect(c.reasons.join(' ')).toMatch(/content rather than from you/i);
  });

  it('does not hard-gate an ordinary file merely because the source was untrusted', () => {
    // The combination is what disqualifies, not the taint alone.
    expect(
      tierFor({ action: 'open', path: '/tmp/notes.pdf', inspection: benign, taint: 'untrusted' }),
    ).not.toBe('hard_gate');
  });
});

/* ------------------------------------------------------------------ */
describe('the card', () => {
  it('leads with the act', () => {
    const { notes } = describeFileAction({ action: 'write', path: '/a/b.txt' });
    expect(notes[0]).toMatch(/\/a\/b\.txt/);
  });

  it('says nothing it was not given', () => {
    const { notes } = describeFileAction({ action: 'list', path: '/a' });
    expect(notes).toHaveLength(1);
  });

  it('never offers "always allow" for an untrusted instruction', () => {
    const { descriptor } = describeFileAction({
      action: 'open',
      path: '/tmp/x.docm',
      inspection: macroDoc,
      taint: 'untrusted',
    });
    expect(classify(descriptor).allowRemember).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('it decides nothing itself', () => {
  it('produces facts only — the tier always comes from the classifier', () => {
    const { descriptor } = describeFileAction({ action: 'read', path: '/a' });
    // No tier, no reasons, no policy on the descriptor.
    expect(Object.keys(descriptor).sort()).toEqual(
      expect.arrayContaining(['args', 'taint', 'tool']),
    );
    expect(descriptor).not.toHaveProperty('tier');
  });
});
