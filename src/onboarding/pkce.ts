/**
 * PKCE for the OpenRouter sign-in flow (RFC 7636).
 *
 * WHY PKCE AND NOT A CLIENT SECRET. **A desktop application cannot keep a
 * secret.** Anything shipped inside the binary is readable by whoever holds the
 * binary, and anything written to disk is readable by the same account the app
 * runs as — which, as the threat model keeps pointing out, is the same account
 * the agent runs as.
 *
 * So this flow uses no client secret at all. The app generates a fresh random
 * verifier for each attempt, sends only its SHA-256 hash, and proves possession
 * of the original at the token exchange. An intercepted authorization code is
 * then useless on its own, because the verifier never left the machine.
 *
 * Everything here is pure and synchronous so it can be tested without a network,
 * a registered client ID, or a browser. The parts that cannot be tested that way
 * — the loopback listener and the token exchange — are deliberately elsewhere.
 */

import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';

/** 256 bits. RFC 7636 requires at least 256 bits of entropy in the verifier. */
const VERIFIER_BYTES = 32;

/** RFC 7636 section 4.1 bounds. */
const MIN_VERIFIER = 43;
const MAX_VERIFIER = 128;

/** The `state` value, which is about request forgery rather than the verifier. */
const STATE_BYTES = 32;

/**
 * base64url WITHOUT padding.
 *
 * Both RFC 7636 and RFC 6749 require this specific alphabet, and getting it
 * wrong is a classic silent failure: the server rejects the challenge with an
 * unhelpful error, or — worse — accepts a value the client computed differently.
 * `+` and `/` are not URL-safe; `=` padding is not allowed.
 */
function base64url(input: Buffer): string {
  return input.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** A fresh verifier. Unreserved characters only, per RFC 7636 section 4.1. */
export function createVerifier(): string {
  return base64url(randomBytes(VERIFIER_BYTES));
}

/**
 * The S256 challenge for a verifier.
 *
 * Note there is deliberately **no `plain` variant**. RFC 7636 permits
 * `code_challenge_method=plain`, which provides essentially no protection
 * against the attack PKCE exists to stop, and the only reason to support it is
 * an old server. We do not have one, so offering it would only be a way to
 * weaken the flow by configuration.
 */
export function challengeFor(verifier: string): string {
  return base64url(createHash('sha256').update(verifier, 'utf8').digest());
}

/** A fresh `state`, used to reject a callback this app did not start. */
export function createState(): string {
  return base64url(randomBytes(STATE_BYTES));
}

/**
 * Constant-time comparison of the returned `state` against the one we sent.
 *
 * Timing-safe because the alternative leaks information about the expected value
 * one byte at a time. It is a small thing, but it is also three lines, and the
 * habit of writing the safe version by default is worth more than the individual
 * attack it prevents.
 */
export function statesMatch(expected: string, received: string): boolean {
  if (typeof expected !== 'string' || typeof received !== 'string') return false;
  if (expected.length === 0 || received.length === 0) return false;
  const left = Buffer.from(expected, 'utf8');
  const right = Buffer.from(received, 'utf8');
  // Length must be compared first: timingSafeEqual throws on a length mismatch
  // rather than returning false, which would turn a rejection into a crash.
  if (left.length !== right.length) return false;
  return timingSafeEqual(left, right);
}

/** Whether a verifier is structurally acceptable to send. Used by tests and by
 *  the exchange step as a last guard before a value leaves the process. */
export function isValidVerifier(verifier: unknown): boolean {
  if (typeof verifier !== 'string') return false;
  if (verifier.length < MIN_VERIFIER || verifier.length > MAX_VERIFIER) return false;
  return /^[A-Za-z0-9\-._~]+$/.test(verifier);
}

export interface PkceAttempt {
  /** Stays on this machine. Never sent until the token exchange. */
  verifier: string;
  /** Sent in the authorization request. */
  challenge: string;
  /** Sent with the request and checked on the way back. */
  state: string;
}

/** Everything one sign-in attempt needs. */
export function createAttempt(): PkceAttempt {
  const verifier = createVerifier();
  return { verifier, challenge: challengeFor(verifier), state: createState() };
}
