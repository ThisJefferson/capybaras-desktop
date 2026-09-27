/**
 * The OpenRouter sign-in flow, minus the socket.
 *
 * Everything here is pure or takes its `fetch` as a parameter, so the whole flow
 * can be exercised without a network, a browser, or a real account. The socket —
 * the loopback listener the browser redirects to — deliberately lives elsewhere,
 * because it is the only part that cannot be tested this way.
 *
 * Verified against OpenRouter's OAuth guide, 2026-09-27.
 */

import { statesMatch } from './pkce';

export const AUTH_ENDPOINT = 'https://openrouter.ai/auth';
export const EXCHANGE_ENDPOINT = 'https://openrouter.ai/api/v1/auth/keys';
export const CALLBACK_PATH = '/callback';

/** The label shown on the key in the user's own OpenRouter dashboard, so it is
 *  identifiable rather than anonymous when they go to revoke it. */
export const KEY_LABEL = 'Capybaras';

/**
 * The loopback address the browser is told to come back to.
 *
 * `localhost` in the URL because that is what the provider documents and titles
 * the app with. The listener must nevertheless bind **127.0.0.1 specifically**,
 * never `0.0.0.0`: binding every interface would put an OAuth callback on the
 * local network, where another machine could race the browser to it.
 */
export function callbackUrl(port: number): string {
  return `http://localhost:${port}${CALLBACK_PATH}`;
}

export interface AuthorizationParams {
  callbackUrl: string;
  /** The S256 challenge. The verifier stays on this machine. */
  challenge: string;
  keyLabel?: string;
}

export function authorizationUrl(params: AuthorizationParams): string {
  const url = new URL(AUTH_ENDPOINT);
  url.searchParams.set('callback_url', params.callbackUrl);
  url.searchParams.set('code_challenge', params.challenge);
  // Never `plain`. See the note in pkce.ts: it provides essentially no
  // protection, and the only reason to offer it is an old server.
  url.searchParams.set('code_challenge_method', 'S256');
  url.searchParams.set('key_label', params.keyLabel ?? KEY_LABEL);
  return url.toString();
}

export type CallbackFailure = 'denied' | 'no-code' | 'state-mismatch' | 'malformed';

export type CallbackResult =
  | { ok: true; code: string }
  | { ok: false; reason: CallbackFailure; detail?: string };

/**
 * Read the browser's redirect.
 *
 * ON `state`: OpenRouter's documentation **does not list a `state` parameter**,
 * so we cannot require one. That is survivable here, and the reasoning is worth
 * writing down rather than leaving as a shrug — because "the provider did not
 * give us a defence, so we skipped it" is how a control quietly disappears.
 *
 * `state` exists to stop login-CSRF: an attacker handing your app *their*
 * authorization code so you end up signed in as them. **PKCE already defeats
 * that**, because exchanging the code requires the verifier, which never leaves
 * this machine and which the attacker does not have. So state is redundant here
 * rather than missing.
 *
 * We still verify it when it *is* present, which costs nothing and would catch a
 * provider that starts sending one.
 */
export function parseCallback(requestUrl: string, expectedState?: string): CallbackResult {
  let url: URL;
  try {
    url = new URL(requestUrl);
  } catch {
    return { ok: false, reason: 'malformed', detail: 'the callback was not a valid URL' };
  }

  // A provider that reports a refusal is not an error in our code, and should
  // read as "you said no" rather than as a failure.
  const providerError = url.searchParams.get('error');
  if (providerError) {
    return {
      ok: false,
      reason: providerError === 'access_denied' ? 'denied' : 'malformed',
      detail: providerError,
    };
  }

  if (expectedState !== undefined) {
    const state = url.searchParams.get('state') ?? '';
    if (!statesMatch(expectedState, state)) {
      return { ok: false, reason: 'state-mismatch' };
    }
  }

  const code = url.searchParams.get('code');
  if (!code) return { ok: false, reason: 'no-code' };

  return { ok: true, code };
}

export type ExchangeFailure =
  | 'expired'
  | 'bad-verifier'
  | 'bad-method'
  | 'bad-request'
  | 'network'
  | 'malformed';

export type ExchangeResult =
  | { ok: true; key: string }
  | { ok: false; reason: ExchangeFailure; detail: string };

/**
 * Trade the code plus the verifier for an API key.
 *
 * `fetchImpl` is injected so this can be tested against every documented failure
 * without touching the network.
 *
 * **Nothing secret is ever put in a `detail` string.** That includes the code
 * the verifier and the key. An error message is the single most likely thing to
 * end up in a log file or a screenshot, so it is the last place a credential
 * should appear — and it is an easy thing to get wrong while writing a helpful
 * message.
 */
export async function exchangeCode(options: {
  code: string;
  verifier: string;
  fetchImpl: typeof fetch;
}): Promise<ExchangeResult> {
  const { code, verifier, fetchImpl } = options;

  let response: Response;
  try {
    response = await fetchImpl(EXCHANGE_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        code,
        code_verifier: verifier,
        code_challenge_method: 'S256',
      }),
    });
  } catch (error) {
    // Deliberately does not interpolate the thrown value: a fetch error can carry
    // the request, and the request contains the code and the verifier.
    return { ok: false, reason: 'network', detail: 'could not reach OpenRouter; check your connection and try again' };
  }

  if (!response.ok) {
    // The documented codes, mapped to something a person can act on.
    if (response.status === 400) {
      return {
        ok: false,
        reason: 'bad-method',
        detail: 'OpenRouter rejected the request: the sign-in was started with a different challenge method than it was finished with',
      };
    }
    if (response.status === 403) {
      // 403 covers both an expired code and a wrong verifier. They need different
      // actions, so try to tell them apart from the body.
      const body = await safeText(response);
      const expired = /expir/i.test(body);
      return expired
        ? { ok: false, reason: 'expired', detail: 'the sign-in took too long and the code has expired; start again' }
        : { ok: false, reason: 'bad-verifier', detail: 'OpenRouter did not accept this sign-in attempt; start again' };
    }
    if (response.status === 405) {
      return { ok: false, reason: 'bad-request', detail: 'OpenRouter rejected the request method' };
    }
    return { ok: false, reason: 'bad-request', detail: `OpenRouter returned an unexpected status (${response.status})` };
  }

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return { ok: false, reason: 'malformed', detail: 'OpenRouter returned a response that could not be read' };
  }

  const key = (body as { key?: unknown } | null)?.key;
  if (typeof key !== 'string' || key.length === 0) {
    return { ok: false, reason: 'malformed', detail: 'OpenRouter returned no key' };
  }

  return { ok: true, key };
}

/** Read a response body for classification only. Never surfaced verbatim. */
async function safeText(response: Response): Promise<string> {
  try {
    return await response.text();
  } catch {
    return '';
  }
}
