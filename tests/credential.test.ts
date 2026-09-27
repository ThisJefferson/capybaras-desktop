import { inspect } from 'node:util';

import { describe, expect, it } from 'vitest';

import {
  InMemoryCredentialStore,
  Secret,
  redact,
} from '../src/onboarding/credential';

/**
 * T12 — the credential the agent can reach.
 *
 * The key is the authority to spend money and read data, and the agent runs as
 * the same user as this app. So a key that reaches a log, a protocol message, an
 * error string or the webview is one the agent can use DIRECTLY, with no gate and
 * no card.
 *
 * These tests are about one property: **the key cannot leak by accident.** It has
 * to be deliberate, through a single greppable word.
 */

// NOTE: this fixture is deliberately NOT shaped like a real key. An earlier
// version used a realistic `sk-or-v1-...` literal, and GitHub's secret scanning
// rejected the push -- correctly. Keep it obviously synthetic.
const KEY = 'test-only-fixture-not-a-real-credential-0000000000000000000000000000000';

describe('a Secret refuses to become text', () => {
  const secret = new Secret(KEY);

  it('does not leak through JSON.stringify', () => {
    // The most likely accident: a payload logged or sent over the protocol.
    expect(JSON.stringify({ key: secret })).toBe('{"key":"[redacted]"}');
    expect(JSON.stringify([secret])).toBe('["[redacted]"]');
    expect(JSON.stringify({ nested: { deep: secret } })).toBe('{"nested":{"deep":"[redacted]"}}');
  });

  it('does not leak through a template literal', () => {
    expect(`connected with ${secret}`).toBe('connected with [redacted]');
  });

  it('does not leak through string concatenation', () => {
    expect('key=' + secret).toBe('key=[redacted]');
  });

  it('does not leak through String()', () => {
    expect(String(secret)).toBe('[redacted]');
  });

  it('does not leak through a logger', () => {
    // Most loggers use util.inspect under the hood.
    expect(inspect(secret)).toBe('[redacted]');
    expect(inspect({ key: secret })).toContain('[redacted]');
    expect(inspect({ key: secret })).not.toContain(KEY);
  });

  it('does not leak when an object containing it is spread or copied', () => {
    const payload = { ...{ key: secret }, status: 'connected' };
    expect(JSON.stringify(payload)).not.toContain(KEY);
  });

  it('reveals the value only when explicitly asked', () => {
    expect(secret.reveal()).toBe(KEY);
  });

  it('reports its length without revealing anything', () => {
    expect(secret.length).toBe(KEY.length);
    expect(String(secret.length)).not.toContain(KEY);
  });

  it('compares without revealing', () => {
    expect(secret.equals(new Secret(KEY))).toBe(true);
    expect(secret.equals(new Secret('a-different-key'))).toBe(false);
  });

  it('refuses to exist when empty, because an empty secret is a bug upstream', () => {
    expect(() => new Secret('')).toThrow();
    expect(() => new Secret(undefined as never)).toThrow();
  });
});

describe('redact', () => {
  it('removes a secret from text that already contains it', () => {
    const text = `Authorization: Bearer ${KEY}`;
    const cleaned = redact(text, KEY);
    expect(cleaned).toBe('Authorization: Bearer [redacted]');
    expect(cleaned).not.toContain(KEY);
  });

  it('accepts a Secret as well as a raw string', () => {
    expect(redact(`x ${KEY} y`, new Secret(KEY))).toBe('x [redacted] y');
  });

  it('removes several secrets at once', () => {
    expect(redact(`${KEY} and sekrit-two`, KEY, 'sekrit-two')).toBe('[redacted] and [redacted]');
  });

  it('leaves text alone when there is nothing to remove', () => {
    expect(redact('nothing secret here', KEY)).toBe('nothing secret here');
  });

  it('ignores an empty needle rather than shredding the string', () => {
    // 'split("")' would insert [redacted] between every character.
    expect(redact('hello', '')).toBe('hello');
  });
});

describe('the store', () => {
  it('round-trips a credential', async () => {
    const store = new InMemoryCredentialStore();
    await store.save('capybaras.oauth.openrouter', new Secret(KEY));
    const loaded = await store.load('capybaras.oauth.openrouter');
    expect(loaded?.reveal()).toBe(KEY);
  });

  it('returns undefined for something never stored, rather than throwing', async () => {
    // "Not connected" is a normal state, not an error.
    expect(await new InMemoryCredentialStore().load('nope')).toBeUndefined();
  });

  it('replaces an existing credential', async () => {
    const store = new InMemoryCredentialStore();
    await store.save('k', new Secret('first'));
    await store.save('k', new Secret('second'));
    expect((await store.load('k'))?.reveal()).toBe('second');
  });

  it('deletes, and deleting something absent is not an error', async () => {
    const store = new InMemoryCredentialStore();
    await store.save('k', new Secret('v'));
    await store.delete('k');
    expect(await store.load('k')).toBeUndefined();
    await expect(store.delete('never-existed')).resolves.toBeUndefined();
  });

  it('forgets everything when it goes away, which is the intended failure direction', () => {
    // An in-memory store means "not connected" after a restart -- never a key
    // written somewhere convenient. There is deliberately no file-backed
    // fallback to reach for.
    const store = new InMemoryCredentialStore();
    expect(store.size()).toBe(0);
  });
});
