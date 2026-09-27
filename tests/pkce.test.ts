import { createHash } from 'node:crypto';

import { describe, expect, it } from 'vitest';

import {
  createAttempt,
  createState,
  createVerifier,
  challengeFor,
  isValidVerifier,
  statesMatch,
} from '../src/onboarding/pkce';

/**
 * PKCE is implemented before anything is connected to a network or to a real
 * client ID, because it is the part where a mistake is both easy to make and
 * invisible: the flow still "works", it is just no longer protecting anything.
 */

function b64url(input: Buffer): string {
  return input.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('the verifier meets RFC 7636', () => {
  it('is within the length bounds', () => {
    const verifier = createVerifier();
    expect(verifier.length).toBeGreaterThanOrEqual(43);
    expect(verifier.length).toBeLessThanOrEqual(128);
  });

  it('uses only unreserved characters', () => {
    for (let i = 0; i < 50; i += 1) {
      expect(createVerifier()).toMatch(/^[A-Za-z0-9\-._~]+$/);
    }
  });

  it('is base64url and NOT base64 — no +, /, or = anywhere', () => {
    // The classic silent failure: standard base64 "works" locally and is
    // rejected by the server, or produces a challenge the server computes
    // differently. Checked across many samples because a short string may simply
    // never contain the offending characters by chance.
    for (let i = 0; i < 200; i += 1) {
      const verifier = createVerifier();
      expect(verifier).not.toMatch(/[+/=]/);
    }
  });

  it('differs every time', () => {
    const seen = new Set(Array.from({ length: 200 }, () => createVerifier()));
    expect(seen.size).toBe(200);
  });

  it('rejects structurally bad verifiers', () => {
    expect(isValidVerifier('too-short')).toBe(false);
    expect(isValidVerifier('a'.repeat(129))).toBe(false);
    expect(isValidVerifier('has spaces in it but is long enough to pass length')).toBe(false);
    expect(isValidVerifier('plus+and/slash+and=equals='.repeat(3))).toBe(false);
    expect(isValidVerifier(undefined)).toBe(false);
    expect(isValidVerifier(createVerifier())).toBe(true);
  });
});

describe('the challenge is S256', () => {
  it('matches a known digest — pinned, so a weakened hash cannot pass', () => {
    // WITHOUT THIS TEST, swapping SHA-256 for something weaker would still have
    // the client and server agree, because both would use the weak function.
    // Agreement proves nothing about strength; a known answer does.
    //
    // RFC 7636 appendix B's vector, re-derived here rather than copied:
    const verifier = 'dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk';
    const expected = b64url(createHash('sha256').update(verifier, 'utf8').digest());
    expect(challengeFor(verifier)).toBe(expected);
    expect(challengeFor(verifier)).toBe('E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM');
  });

  it('is base64url, not base64', () => {
    for (let i = 0; i < 200; i += 1) {
      expect(challengeFor(createVerifier())).not.toMatch(/[+/=]/);
    }
  });

  it('is deterministic for a given verifier', () => {
    const verifier = createVerifier();
    expect(challengeFor(verifier)).toBe(challengeFor(verifier));
  });

  it('differs for different verifiers', () => {
    expect(challengeFor(createVerifier())).not.toBe(challengeFor(createVerifier()));
  });

  it('is a 256-bit digest, so 43 base64url characters', () => {
    // 32 bytes -> 43 unpadded base64url characters. A shorter value would mean a
    // truncated hash, which is the kind of thing that still "works".
    expect(challengeFor(createVerifier()).length).toBe(43);
  });
});

describe('state', () => {
  it('is fresh every time and long enough to be unguessable', () => {
    const seen = new Set(Array.from({ length: 200 }, () => createState()));
    expect(seen.size).toBe(200);
    expect(createState().length).toBeGreaterThanOrEqual(43);
  });

  it('matches only the exact value', () => {
    const state = createState();
    expect(statesMatch(state, state)).toBe(true);
    expect(statesMatch(state, createState())).toBe(false);
    expect(statesMatch(state, state.slice(0, -1))).toBe(false);
    expect(statesMatch(state, `${state}x`)).toBe(false);
  });

  it('rejects empty and missing values rather than treating them as equal', () => {
    // The dangerous degenerate case: two absent values comparing equal is a
    // request-forgery check that passes for every attacker who sends nothing.
    expect(statesMatch('', '')).toBe(false);
    expect(statesMatch('', createState())).toBe(false);
    expect(statesMatch(createState(), '')).toBe(false);
    expect(statesMatch(undefined as never, undefined as never)).toBe(false);
  });
});

describe('an attempt', () => {
  it('carries a valid verifier and the challenge that matches it', () => {
    const attempt = createAttempt();
    expect(isValidVerifier(attempt.verifier)).toBe(true);
    expect(attempt.challenge).toBe(challengeFor(attempt.verifier));
    expect(attempt.state.length).toBeGreaterThanOrEqual(43);
  });

  it('never repeats a verifier or a state across attempts', () => {
    const verifiers = new Set<string>();
    const states = new Set<string>();
    for (let i = 0; i < 100; i += 1) {
      const attempt = createAttempt();
      verifiers.add(attempt.verifier);
      states.add(attempt.state);
    }
    expect(verifiers.size).toBe(100);
    expect(states.size).toBe(100);
  });
});
