import { describe, expect, it } from 'vitest';

import {
  CREDENTIAL_NAME_PREFIX,
  PROVIDERS,
  connectableProviders,
  credentialNameFor,
  providerFor,
  validateProviders,
} from '../src/onboarding/providers';

/** A known-good provider to mutate for the refusal tests. The registry is a
 *  non-empty literal, so the assertion is safe — and `PROVIDERS[0]` alone is
 *  `Provider | undefined` under noUncheckedIndexedAccess. */
const good = PROVIDERS[0]!;

describe('the registry as declared', () => {
  it('is valid', () => {
    expect(validateProviders(PROVIDERS)).toEqual([]);
  });

  it('offers OpenRouter', () => {
    const openrouter = providerFor('openrouter');
    expect(openrouter).toBeDefined();
    expect(openrouter?.name).toBe('OpenRouter');
    expect(openrouter?.auth).toBe('oauth');
  });

  it('returns undefined for a provider nobody declared', () => {
    expect(providerFor('not-a-provider')).toBeUndefined();
  });

  it('only ever returns OAuth providers to the menu', () => {
    for (const provider of connectableProviders()) {
      expect(provider.auth).toBe('oauth');
    }
  });

  it('does not instruct anyone to paste a key', () => {
    // The help text is the instruction a non-technical user follows. If it ever
    // mentions an API key, the product has quietly gone back to asking for one.
    for (const provider of PROVIDERS) {
      expect(provider.howToConnect).not.toMatch(/\bapi[_ -]?key\b/i);
      expect(provider.howToConnect.toLowerCase()).not.toContain('paste');
    }
  });

  it('uses https for everything it links or fetches', () => {
    for (const provider of PROVIDERS) {
      expect(provider.docsUrl.startsWith('https://')).toBe(true);
      expect(provider.modelsEndpoint.startsWith('https://')).toBe(true);
    }
  });
});

describe('credential naming', () => {
  it('namespaces the stored credential so it is identifiable', () => {
    expect(credentialNameFor('openrouter')).toBe(`${CREDENTIAL_NAME_PREFIX}.openrouter`);
  });

  it('keeps one namespace across providers', () => {
    expect(CREDENTIAL_NAME_PREFIX).toMatch(/^capybaras\./);
  });
});

/**
 * THE SCOPE GUARD, and why these tests exist.
 *
 * Jeff's decision was: only OAuth models, so **no key pasting anywhere**. The
 * risk is not that someone disagrees — it is that six weeks from now someone adds
 * "just one more provider", pastes a key into the state directory, and the
 * product's central promise becomes optional without anyone deciding that.
 *
 * These tests exist so that change cannot happen quietly. They prove the guard
 * can REFUSE, which is the only thing that makes it a guard rather than a
 * comment.
 */
describe('validation refuses a provider that would change the promise', () => {
  const patch = (fields: Record<string, unknown>) =>
    validateProviders([{ ...good, ...fields } as never]);

  it('refuses a provider that needs a pasted key', () => {
    const problems = patch({ auth: 'key' });
    expect(problems).not.toEqual([]);
    expect(problems.join(' ')).toMatch(/only OAuth providers/i);
  });

  it('explains WHY it refused, and points at the threat model', () => {
    // A refusal that does not say why gets worked around rather than understood.
    const problems = patch({ auth: 'apikey' }).join(' ');
    expect(problems).toMatch(/T12/);
    expect(problems).toMatch(/same user/i);
  });

  it('refuses help text that tells the user to paste a key', () => {
    expect(patch({ howToConnect: 'Paste your API key here' })).not.toEqual([]);
    expect(patch({ howToConnect: 'Enter your api-key to continue' })).not.toEqual([]);
  });
});

describe('validation refuses a malformed entry', () => {
  const patch = (fields: Record<string, unknown>) =>
    validateProviders([{ ...good, ...fields } as never]);

  it.each([
    ['missing id', { id: '' }],
    ['missing name', { name: '' }],
    ['missing help text', { howToConnect: '   ' }],
    ['missing docs link', { docsUrl: undefined }],
    ['missing models endpoint', { modelsEndpoint: undefined }],
    ['a plain-http docs link', { docsUrl: 'http://example.com' }],
    ['a plain-http models endpoint', { modelsEndpoint: 'http://example.com/models' }],
  ])('refuses %s', (_label, fields) => {
    expect(patch(fields)).not.toEqual([]);
  });

  it('refuses a duplicate id', () => {
    expect(validateProviders([good, good])).not.toEqual([]);
  });

  it('refuses something that is not an object, without throwing', () => {
    // A validator that crashes on malformed input is close to useless, because
    // malformed input is its entire job.
    expect(validateProviders([null as never])).not.toEqual([]);
    expect(validateProviders([undefined as never])).not.toEqual([]);
    expect(validateProviders(['string' as never])).not.toEqual([]);
  });
});
