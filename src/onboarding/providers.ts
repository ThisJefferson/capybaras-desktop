/**
 * The provider registry.
 *
 * Declarative, like the skill registry, and for the same reason: the knowledge
 * should live in one reviewable list rather than be scattered through the code
 * that uses it.
 *
 * **THE SCOPE DECISION IS ENFORCED HERE, NOT REMEMBERED.** Jeff's call: only
 * models reachable through OAuth are listed, so there is no key-pasting anywhere
 * in the product. This module encodes that as a **type** — `auth` can only be
 * `'oauth'` — so adding a provider that needs a pasted key is not a configuration
 * change. It is a deliberate edit to a union, which fails validation until
 * someone states out loud that they are introducing credential handling.
 *
 * That matters because the alternative is exactly how a decision gets quietly
 * reversed six weeks later: someone adds one provider, pastes one key into the
 * state directory, and the product's central promise becomes optional. A build
 * failure is a much better place to have that argument.
 */

/**
 * How a provider authenticates.
 *
 * `'key'` is deliberately NOT in this union. A provider requiring a pasted
 * credential cannot be added without editing this line — and `validateProviders`
 * rejects anything that is not `'oauth'`, so the edit alone is not enough to make
 * it ship by accident.
 */
export type ProviderAuth = 'oauth';

export interface Provider {
  /** Stable id, used in messages and for the stored credential's name. */
  id: string;
  /** Shown to the user. */
  name: string;
  /** How the user connects, in OUR words. Never fetched from anywhere. */
  howToConnect: string;
  /** Where to read more. A link we choose, not content we render. */
  docsUrl: string;
  /** The endpoint that lists this provider's models. */
  modelsEndpoint: string;
  /** Whether this provider's models are reachable once connected. */
  auth: ProviderAuth;
}

/** The label a stored credential is filed under, so the user can find it. */
export const CREDENTIAL_NAME_PREFIX = 'capybaras.oauth';

export function credentialNameFor(providerId: string): string {
  return `${CREDENTIAL_NAME_PREFIX}.${providerId}`;
}

export const PROVIDERS: readonly Provider[] = Object.freeze([
  {
    id: 'openrouter',
    name: 'OpenRouter',
    // Written for someone who has never heard the phrase "API key". The whole
    // point of the OAuth path is that this sentence is the only instruction they
    // need.
    howToConnect:
      'Click Connect, then sign in to OpenRouter in your browser and press Authorize. ' +
      'Nothing to copy, and no password is typed into this app.',
    docsUrl: 'https://openrouter.ai/docs/guides/overview/auth/oauth',
    modelsEndpoint: 'https://openrouter.ai/api/v1/models',
    auth: 'oauth',
  },
]);

const BY_ID = new Map(PROVIDERS.map((provider) => [provider.id, provider]));

export function providerFor(id: string): Provider | undefined {
  return BY_ID.get(id);
}

/** Every provider the settings menu should offer. */
export function connectableProviders(): Provider[] {
  return PROVIDERS.filter((provider) => provider.auth === 'oauth');
}

/**
 * Check the registry itself. Returns problems; empty means valid.
 *
 * A provider that has not declared how the user authenticates must not be able to
 * ship — the same rule the skill registry applies, for the same reason: a
 * missing declaration becomes a lenient default, and a lenient default in a
 * security-adjacent list is how a control disappears.
 */
export function validateProviders(providers: readonly Provider[] = PROVIDERS): string[] {
  const problems: string[] = [];
  const seen = new Set<string>();

  for (const [index, provider] of providers.entries()) {
    const usable = typeof provider?.id === 'string' && provider.id.length > 0;
    const where = usable ? `provider "${provider.id}"` : `provider at index ${index}`;

    if (provider === null || provider === undefined || typeof provider !== 'object') {
      problems.push(`provider at index ${index}: not an object`);
      continue;
    }

    if (!usable) {
      problems.push(`${where}: missing id, or not a string`);
    } else if (seen.has(provider.id)) {
      problems.push(`${where}: duplicate id`);
    }
    if (usable) seen.add(provider.id);

    for (const field of ['name', 'howToConnect', 'docsUrl', 'modelsEndpoint'] as const) {
      const value = provider[field];
      if (typeof value !== 'string' || value.trim().length === 0) {
        problems.push(`${where}: missing ${field}`);
      }
    }

    // THE SCOPE GUARD. Anything that is not OAuth is a change to the product's
    // promise, not a configuration detail.
    if (provider.auth !== 'oauth') {
      problems.push(
        `${where}: auth is "${String(provider.auth)}" -- only OAuth providers may be listed. ` +
          'A provider needing a pasted key reintroduces credential handling, and the agent runs ' +
          'as the same user as this app (docs/threat-model.md, T12).',
      );
    }

    // A help string that names a key would be instructing the user to paste one.
    if (typeof provider.howToConnect === 'string' && /\bapi[_ -]?key\b/i.test(provider.howToConnect)) {
      problems.push(`${where}: howToConnect mentions an API key, which this path does not use`);
    }

    if (typeof provider.docsUrl === 'string' && !/^https:\/\//.test(provider.docsUrl)) {
      problems.push(`${where}: docsUrl must be https`);
    }

    if (typeof provider.modelsEndpoint === 'string' && !/^https:\/\//.test(provider.modelsEndpoint)) {
      problems.push(`${where}: modelsEndpoint must be https`);
    }
  }

  return problems;
}
