import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { GLOSSARY, HARD_GATE_NOTE, explain, tierCopy } from '../src/policy';

/**
 * The card's voice (gap 7.7).
 *
 * The card is the product, and it is the one screen that has to work for someone
 * who has never read our threat model. This file asserts two things with teeth:
 *
 *   1. The copy module says what a person would say.
 *   2. **The frontend does not speak our language.** That second one is the
 *      reason the file exists — a test on a module cannot stop someone editing
 *      `app.js` directly, and that is exactly how "Hard gate" got onto the card
 *      in the first place.
 */

const appJs = readFileSync(
  join(__dirname, '..', 'apps', 'desktop', 'web', 'app.js'),
  'utf8',
);

describe('what the card calls a tier', () => {
  it('does not label the tiers that are not decisions', () => {
    expect(tierCopy('silent')).toBeNull();
    expect(tierCopy('notify')).toBeNull();
    expect(tierCopy('confirm')).toBeNull();
  });

  it('describes the hard gate without naming the mechanism', () => {
    const copy = tierCopy('hard_gate');
    expect(copy).not.toBeNull();
    expect(copy?.label).toBe('Needs care');
    expect(copy?.label.toLowerCase()).not.toContain('gate');
  });

  it('tells the person what will actually be different', () => {
    const copy = tierCopy('hard_gate');
    // Both halves are things they will experience: they type, and it is not
    // remembered. That is what makes the sentence worth its space.
    expect(copy?.note).toMatch(/type/i);
    expect(copy?.note).toMatch(/remember/i);
  });
});

describe('the glossary is a safety net, not a vocabulary', () => {
  it('explains every term in plain words', () => {
    for (const { term, plain } of GLOSSARY) {
      expect(plain.length, `${term} has no explanation`).toBeGreaterThan(10);
      // A definition that uses the word it defines explains nothing.
      const words = plain.toLowerCase().split(/\W+/);
      expect(words, `${term} is defined using itself`).not.toContain(term.toLowerCase());
    }
  });

  it('resolves a term regardless of case or plural', () => {
    expect(explain('hard gate')).toMatch(/never make on its own/i);
    expect(explain('Hard Gates')).toMatch(/never make on its own/i);
    expect(explain('REVERSIBLE')).toMatch(/undone/i);
  });

  it('prefers the longer term, so a phrase is not caught by a fragment', () => {
    expect(explain('blast radius')).toMatch(/how many things/i);
  });

  it('returns nothing rather than guessing', () => {
    expect(explain('splunge')).toBeNull();
    expect(explain('')).toBeNull();
    expect(explain('   ')).toBeNull();
  });

  it('does not match a term buried inside another word', () => {
    // "tainted" is a real word but it is not the identifier we mean, and a
    // confident wrong explanation on the card is worse than none.
    expect(explain('untainted')).toBeNull();
  });
});

describe('the frontend does not speak our language', () => {
  /** The words we use about ourselves, which a person should never meet. */
  const OUR_WORDS = [
    'hard gate',
    'hard_gate',
    'blast radius',
    'protected target',
    'exfiltration',
  ];

  it('the card copy in app.js contains none of our vocabulary', () => {
    // Only the strings the person reads. Identifiers like `hard_gate` in
    // comparisons are allowed — this looks for the phrase in quotes, which is
    // how a label reaches the screen.
    for (const word of OUR_WORDS) {
      const asLabel = new RegExp(`textContent\\s*=\\s*['"\`][^'"\`]*${word}[^'"\`]*['"\`]`, 'i');
      expect(asLabel.test(appJs), `app.js renders "${word}" as a label`).toBe(false);
    }
  });

  it('says "Needs care" where the tier is shown', () => {
    expect(appJs).toContain('Needs care');
  });

  it('keeps the card copy in step with the module', () => {
    // The literal in the frontend and the constant here are the same sentence.
    // If they drift, a test fails rather than a person reading two voices.
    const note = HARD_GATE_NOTE.trim();
    const firstClause = note.split(',')[0];
    expect(appJs).toContain(firstClause);
  });
});
