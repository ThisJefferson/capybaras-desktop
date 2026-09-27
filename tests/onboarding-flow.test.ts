import { describe, expect, it } from 'vitest';

import {
  AUTH_ENDPOINT,
  EXCHANGE_ENDPOINT,
  authorizationUrl,
  callbackUrl,
  exchangeCode,
  parseCallback,
} from '../src/onboarding/flow';
import { createAttempt } from '../src/onboarding/pkce';

/** A minimal stand-in for a fetch Response. */
function fakeResponse(status: number, body: unknown, rawText?: string): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
    text: async () => rawText ?? JSON.stringify(body),
  } as unknown as Response;
}

const returning = (response: Response): typeof fetch => (async () => response) as typeof fetch;
const throwing = (message: string): typeof fetch =>
  (async () => {
    throw new Error(message);
  }) as typeof fetch;

describe('the loopback callback address', () => {
  it('points at localhost and our own path', () => {
    const url = new URL(callbackUrl(51423));
    expect(url.hostname).toBe('localhost');
    expect(url.port).toBe('51423');
    expect(url.pathname).toBe('/callback');
    expect(url.protocol).toBe('http:');
  });
});

describe('the authorization URL', () => {
  const attempt = createAttempt();
  const url = new URL(
    authorizationUrl({ callbackUrl: callbackUrl(51423), challenge: attempt.challenge }),
  );

  it('goes to OpenRouter\u2019s auth endpoint', () => {
    expect(`${url.origin}${url.pathname}`).toBe(AUTH_ENDPOINT);
  });

  it('carries the callback, the challenge, and S256', () => {
    expect(url.searchParams.get('callback_url')).toBe(callbackUrl(51423));
    expect(url.searchParams.get('code_challenge')).toBe(attempt.challenge);
    expect(url.searchParams.get('code_challenge_method')).toBe('S256');
  });

  it('never asks for the "plain" method', () => {
    // `plain` provides essentially no protection against the attack PKCE exists
    // to stop. It must not be reachable by configuration either.
    expect(url.searchParams.get('code_challenge_method')).not.toBe('plain');
  });

  it('labels the key so the user can find it in their own dashboard', () => {
    expect(url.searchParams.get('key_label')).toBe('Capybaras');
  });

  it('sends the challenge, never the verifier', () => {
    // The single most important property of the whole flow.
    expect(url.toString()).not.toContain(attempt.verifier);
  });
});

describe('reading the redirect', () => {
  const state = createAttempt().state;

  it('accepts a code when the state matches', () => {
    const result = parseCallback(`http://localhost:1234/callback?code=abc123&state=${state}`, state);
    expect(result).toEqual({ ok: true, code: 'abc123' });
  });

  /**
   * The ORDER is deliberate, and worth pinning because it was found by accident.
   * An earlier version of the two tests around this one passed an expected state
   * while sending a callback WITHOUT one, and got state-mismatch where they
   * expected a code. That behaviour is correct -- a request that is not ours must
   * not have its code read at all -- so the tests were fixed, not the code.
   */
  it('checks the state BEFORE touching the code', () => {
    const result = parseCallback('http://localhost:1234/callback?code=abc123', state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('state-mismatch');
  });

  it('reads a refusal as a refusal, not a failure', () => {
    const result = parseCallback('http://localhost:1234/callback?error=access_denied', state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('denied');
  });

  it('refuses a callback with no code', () => {
    // A valid state, no code -- so this reaches the code check rather than
    // stopping at the state check above.
    const result = parseCallback(`http://localhost:1234/callback?state=${state}`, state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('no-code');
  });

  it('refuses a wrong state when one is expected', () => {
    const result = parseCallback('http://localhost:1234/callback?code=abc&state=wrong', state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('state-mismatch');
  });

  it('refuses a missing state when one is expected', () => {
    const result = parseCallback('http://localhost:1234/callback?code=abc', state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('state-mismatch');
  });

  it('treats a malformed URL as malformed rather than throwing', () => {
    const result = parseCallback('not a url at all', state);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('malformed');
  });

  /**
   * OpenRouter does not document a `state` parameter, so we cannot require one.
   * The reasoning is worth pinning in a test, because "the provider gave us no
   * defence so we skipped it" is how a control quietly disappears.
   *
   * `state` exists to stop login-CSRF: an attacker handing us THEIR code so we
   * end up signed in as them. PKCE already defeats that -- exchanging the code
   * needs the verifier, which never leaves this machine. State is redundant, not
   * absent.
   */
  it('proceeds without a state, because PKCE already covers that attack', () => {
    const result = parseCallback('http://localhost:1234/callback?code=abc123');
    expect(result).toEqual({ ok: true, code: 'abc123' });
  });
});

describe('exchanging the code', () => {
  const CODE = 'the-authorization-code';
  const VERIFIER = 'the-verifier-that-must-never-leak';

  it('returns the key on success', async () => {
    const result = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: returning(fakeResponse(200, { key: 'sk-or-v1-abc', user_id: 'u1' })),
    });
    expect(result).toEqual({ ok: true, key: 'sk-or-v1-abc' });
  });

  it('POSTs JSON containing the code and the verifier', async () => {
    let seen: { url: string; init: RequestInit } | undefined;
    const spy = (async (url: string, init: RequestInit) => {
      seen = { url, init };
      return fakeResponse(200, { key: 'k' });
    }) as unknown as typeof fetch;

    await exchangeCode({ code: CODE, verifier: VERIFIER, fetchImpl: spy });

    expect(seen?.url).toBe(EXCHANGE_ENDPOINT);
    expect(seen?.init.method).toBe('POST');
    const body = JSON.parse(String(seen?.init.body));
    expect(body.code).toBe(CODE);
    expect(body.code_verifier).toBe(VERIFIER);
    expect(body.code_challenge_method).toBe('S256');
  });

  it.each([
    [400, 'bad-method'],
    [405, 'bad-request'],
    [500, 'bad-request'],
  ])('maps status %i to %s', async (status, reason) => {
    const result = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: returning(fakeResponse(status, { error: 'nope' })),
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe(reason);
  });

  it('distinguishes an expired code from a bad verifier, because the fix differs', async () => {
    const expired = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: returning(fakeResponse(403, { error: 'expired' }, 'Authorization code expired')),
    });
    expect(expired.ok).toBe(false);
    if (!expired.ok) {
      expect(expired.reason).toBe('expired');
      expect(expired.detail).toMatch(/start again/i);
    }

    const wrong = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: returning(fakeResponse(403, { error: 'invalid' }, 'Invalid code')),
    });
    expect(wrong.ok).toBe(false);
    if (!wrong.ok) expect(wrong.reason).toBe('bad-verifier');
  });

  it('treats an unreachable network as a network problem', async () => {
    const result = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: throwing('getaddrinfo ENOTFOUND openrouter.ai'),
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('network');
  });

  it('refuses a response with no key rather than treating it as connected', async () => {
    const result = await exchangeCode({
      code: CODE,
      verifier: VERIFIER,
      fetchImpl: returning(fakeResponse(200, { user_id: 'u1' })),
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('malformed');
  });

  it('refuses a 200 whose body is not JSON', async () => {
    const broken = {
      ok: true,
      status: 200,
      json: async () => {
        throw new Error('not json');
      },
      text: async () => 'not json',
    } as unknown as Response;
    const result = await exchangeCode({ code: CODE, verifier: VERIFIER, fetchImpl: returning(broken) });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('malformed');
  });

  /**
   * THE TEST THAT MATTERS MOST HERE.
   *
   * An error message is the single most likely thing to end up in a log file, a
   * screenshot, or a bug report. So it is the last place a credential should
   * appear -- and it is an easy thing to get wrong while writing a helpful
   * message, which is exactly why this is asserted rather than intended.
   */
  it('never puts the code, the verifier, or the thrown error in a message', async () => {
    const failures = [
      returning(fakeResponse(400, {})),
      returning(fakeResponse(403, {}, 'something mentioning the code')),
      returning(fakeResponse(405, {})),
      returning(fakeResponse(500, {})),
      throwing(`failed while sending ${CODE} and ${VERIFIER}`),
      returning(fakeResponse(200, {})),
    ];

    for (const fetchImpl of failures) {
      const result = await exchangeCode({ code: CODE, verifier: VERIFIER, fetchImpl });
      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.detail).not.toContain(CODE);
        expect(result.detail).not.toContain(VERIFIER);
        expect(result.detail).not.toBe('');
      }
    }
  });
});
