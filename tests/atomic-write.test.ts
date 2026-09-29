import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { writeAtomic, writeNewOnly } from '../src/atomic-write';

/**
 * T15 — the write itself.
 *
 * The approval card decides *whether* something is written. This file is about
 * what happens when it is: a crash must not leave half a document, and nothing
 * must be replaced that a person did not agree to replace.
 *
 * The tests run against a real temporary directory rather than a mock, because
 * the guarantee being made is a filesystem guarantee — `rename` being atomic,
 * `'wx'` refusing an existing file. A mocked `fs` would test that we call the
 * right functions, which is not the same claim and would pass while the product
 * ate someone's work.
 */

let dir: string;

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), 'capybaras-atomic-'));
});

afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const p = (name: string) => join(dir, name);
const leftovers = () => readdirSync(dir).filter((f) => f.endsWith('.tmp'));

/* ------------------------------------------------------------------ */
describe('writeAtomic — replaces, and leaves nothing half-written', () => {
  it('creates a file that is not there', () => {
    const out = writeAtomic(p('new.txt'), 'hello');
    expect(out.ok).toBe(true);
    expect(readFileSync(p('new.txt'), 'utf8')).toBe('hello');
  });

  it('replaces a file that is', () => {
    writeFileSync(p('f.txt'), 'old');
    expect(writeAtomic(p('f.txt'), 'new').ok).toBe(true);
    expect(readFileSync(p('f.txt'), 'utf8')).toBe('new');
  });

  it('leaves no temporary file behind on success', () => {
    writeAtomic(p('f.txt'), 'x');
    expect(leftovers()).toEqual([]);
  });

  it('leaves the ORIGINAL untouched when the write cannot happen', () => {
    writeFileSync(p('f.txt'), 'precious');
    // A directory in the target's place: the rename must fail rather than
    // clobber, and the failure must be a value rather than a throw.
    const out = writeAtomic(join(dir, 'f.txt', 'nested'), 'x');
    expect(out.ok).toBe(false);
    expect(readFileSync(p('f.txt'), 'utf8')).toBe('precious');
  });

  it('leaves no temporary file behind on failure either', () => {
    writeAtomic(join(dir, 'missing-dir', 'f.txt'), 'x');
    expect(leftovers()).toEqual([]);
  });

  it('round-trips content that is not ASCII', () => {
    const text = 'café — naïve · 日本語 · emoji 🦫';
    writeAtomic(p('u.txt'), text);
    expect(readFileSync(p('u.txt'), 'utf8')).toBe(text);
  });

  it('writes the whole thing, not a prefix, for a large payload', () => {
    const big = 'line\n'.repeat(50_000);
    writeAtomic(p('big.txt'), big);
    expect(readFileSync(p('big.txt'), 'utf8')).toHaveLength(big.length);
  });
});

/* ------------------------------------------------------------------ */
describe('writeNewOnly — an existing file is a question', () => {
  it('creates a file that is not there', () => {
    const out = writeNewOnly(p('new.txt'), 'hello');
    expect(out.ok).toBe(true);
    expect(readFileSync(p('new.txt'), 'utf8')).toBe('hello');
  });

  it('refuses when something is already there, and says which', () => {
    writeFileSync(p('taken.txt'), 'someone else’s work');
    const out = writeNewOnly(p('taken.txt'), 'mine');
    expect(out.ok).toBe(false);
    if (!out.ok) expect(out.reason).toBe('exists');
  });

  it('DOES NOT TOUCH the file it refused to replace', () => {
    // This is the whole point of the function, and the assertion that matters
    // most in this file: a refusal must be perfectly safe, or "we asked first"
    // means nothing.
    writeFileSync(p('taken.txt'), 'someone else’s work');
    writeNewOnly(p('taken.txt'), 'mine');
    expect(readFileSync(p('taken.txt'), 'utf8')).toBe('someone else’s work');
  });

  it('leaves no temporary file behind when it refuses', () => {
    writeFileSync(p('taken.txt'), 'x');
    writeNewOnly(p('taken.txt'), 'y');
    expect(leftovers()).toEqual([]);
  });

  it('refuses an empty target the same way', () => {
    writeFileSync(p('empty.txt'), '');
    const out = writeNewOnly(p('empty.txt'), 'now with content');
    expect(out.ok).toBe(false);
    expect(readFileSync(p('empty.txt'), 'utf8')).toBe('');
  });

  it('succeeds on the second attempt at a different path', () => {
    writeFileSync(p('a.txt'), 'x');
    expect(writeNewOnly(p('a.txt'), 'y').ok).toBe(false);
    expect(writeNewOnly(p('b.txt'), 'y').ok).toBe(true);
  });
});

/* ------------------------------------------------------------------ */
describe('the two are not interchangeable', () => {
  it('one replaces and the other refuses — same file, opposite outcomes', () => {
    writeFileSync(p('f.txt'), 'first');
    expect(writeNewOnly(p('f.txt'), 'second').ok).toBe(false);
    expect(readFileSync(p('f.txt'), 'utf8')).toBe('first');
    expect(writeAtomic(p('f.txt'), 'second').ok).toBe(true);
    expect(readFileSync(p('f.txt'), 'utf8')).toBe('second');
  });

  it('a failure is a value, never an exception', () => {
    // Both must be callable in a context where throwing would crash the shell.
    expect(() => writeAtomic(join(dir, 'nope', 'x'), 'a')).not.toThrow();
    expect(() => writeNewOnly(join(dir, 'nope', 'x'), 'a')).not.toThrow();
  });
});
