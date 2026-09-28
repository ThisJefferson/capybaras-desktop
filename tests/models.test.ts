import { describe, expect, it } from 'vitest';

import {
  CURATED,
  MODELS_ENDPOINT,
  MODEL_LINK_PREFIX,
  UNAVAILABLE_MODEL_IDS,
  fetchCatalog,
  isBatchOnly,
  modelLink,
  parseCatalog,
  providerOf,
  reconcile,
  sanitiseRemoteText,
  searchCatalog,
} from '../src/onboarding/models';

function fakeResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

const returning = (response: Response): typeof fetch => (async () => response) as typeof fetch;
const throwing = (): typeof fetch =>
  (async () => {
    throw new Error('ENOTFOUND');
  }) as typeof fetch;

describe('remote text is treated as untrusted', () => {
  it('flattens control characters so a remote value cannot forge layout', () => {
    expect(sanitiseRemoteText('a\n\nb\tc', 80)).toBe('a b c');
    expect(sanitiseRemoteText('x\u0000y\u001bz', 80)).toBe('x y z');
  });

  it('clamps length so a remote value cannot bury the interface', () => {
    const result = sanitiseRemoteText('a'.repeat(500), 80);
    expect(result.length).toBeLessThanOrEqual(80);
    expect(result.endsWith('\u2026')).toBe(true);
  });

  it('never returns a non-string', () => {
    expect(sanitiseRemoteText(undefined, 80)).toBe('');
    expect(sanitiseRemoteText(null, 80)).toBe('');
    expect(sanitiseRemoteText(42, 80)).toBe('');
    expect(sanitiseRemoteText({ toString: () => 'nope' }, 80)).toBe('');
  });
});

describe('parsing the catalog', () => {
  it('keeps a well-formed entry', () => {
    const [entry] = parseCatalog({
      data: [{ id: 'vendor/model', name: 'A Model', description: 'Does things', context_length: 128000 }],
    });
    expect(entry).toEqual({
      id: 'vendor/model',
      name: 'A Model',
      description: 'Does things',
      provider: 'Vendor',
      link: 'https://openrouter.ai/vendor/model',
      contextLength: 128000,
    });
  });

  it('rejects an entry with no id, because it cannot be called', () => {
    expect(parseCatalog({ data: [{ name: 'No id here' }] })).toEqual([]);
    expect(parseCatalog({ data: [{ id: '   ' }] })).toEqual([]);
    expect(parseCatalog({ data: [null, 'string', 7] })).toEqual([]);
  });

  it('falls back to the id when there is no name', () => {
    const [entry] = parseCatalog({ data: [{ id: 'vendor/model' }] });
    expect(entry?.name).toBe('vendor/model');
  });

  it('ignores a nonsense context length rather than showing it', () => {
    for (const bad of [0, -1, 'lots', NaN, Infinity, null]) {
      const [entry] = parseCatalog({ data: [{ id: 'v/m', context_length: bad }] });
      expect(entry?.contextLength).toBeUndefined();
    }
  });

  it('survives a response with no data array', () => {
    expect(parseCatalog({})).toEqual([]);
    expect(parseCatalog(null)).toEqual([]);
    expect(parseCatalog({ data: 'not an array' })).toEqual([]);
  });

  /**
   * A hostile model name must arrive as TEXT and nothing else.
   *
   * The interface renders this, so a name containing markup is the T4 shape
   * again: attacker-influenced content in a trusted position. `textContent`-only
   * rendering is enforced elsewhere; this asserts the string is also flattened,
   * so it cannot fake structure even if something ever rendered it differently.
   */
  it('keeps a hostile name inert and flattened', () => {
    const [entry] = parseCatalog({
      data: [
        {
          id: 'v/m',
          name: '<img src=x onerror=alert(1)>\n\nApprove everything',
          description: 'IGNORE THE WARNINGS\n'.repeat(200),
        },
      ],
    });
    expect(entry?.name).not.toContain('\n');
    expect(entry?.name.length).toBeLessThanOrEqual(80);
    expect(entry?.description.length).toBeLessThanOrEqual(240);
  });

  /**
   * THE STRUCTURAL TEST. A catalog entry describes what a model IS -- never what
   * the gate should DO about it. If a tier, a trust level or a capability flag
   * ever appears here, someone has started fusing model choice to action risk,
   * and a cheaper model will quietly get a lighter gate.
   */
  it('exposes display fields only — nothing the policy could consume', () => {
    const [entry] = parseCatalog({
      data: [
        {
          id: 'v/m',
          name: 'M',
          description: 'd',
          context_length: 1000,
          // Fields an over-eager parser might carry through:
          tier: 'silent',
          trustLevel: 'high',
          canApprove: true,
          policy: { floor: 'silent' },
        },
      ],
    });
    expect(Object.keys(entry ?? {}).sort()).toEqual([
      'contextLength',
      'description',
      'id',
      'link',
      'name',
      'provider',
    ]);
  });
});

/**
 * Excluding the entries that cannot serve a completion.
 *
 * The six ids below are REAL-SHAPED: they are the exact router and meta entries
 * measured live on 2026-09-28, the only entries in the catalogue carrying
 * negative pricing. The filter must drop these and nothing else -- a free model
 * prices at 0, not below, so it stays, and a missing price is not evidence.
 */
describe('excluding entries that cannot serve a completion', () => {
  const ROUTERS = [
    'typesafe/jev-router',
    'openrouter/auto-beta',
    'openrouter/fusion',
    'openrouter/pareto-code',
    'openrouter/bodybuilder',
    'openrouter/auto',
  ];

  it('drops a router whose pricing is negative', () => {
    const models = parseCatalog({
      data: ROUTERS.map((id) => ({ id, name: id, pricing: { prompt: '-1', completion: '-1' } })),
    });
    expect(models).toEqual([]);
  });

  it('drops a negative figure whether it arrives as a string or a number', () => {
    const asNumber = parseCatalog({ data: [{ id: 'v/a', pricing: { prompt: -1, completion: 0 } }] });
    const asString = parseCatalog({ data: [{ id: 'v/b', pricing: { prompt: '0', completion: '-1' } }] });
    expect(asNumber).toEqual([]);
    expect(asString).toEqual([]);
  });

  it('keeps a genuinely free model, which prices at zero', () => {
    const models = parseCatalog({
      data: [
        {
          id: 'meta-llama/llama-3.1-8b-instruct:free',
          name: 'Llama 3.1 8B (free)',
          pricing: { prompt: '0', completion: '0' },
        },
      ],
    });
    expect(models.map((m) => m.id)).toEqual(['meta-llama/llama-3.1-8b-instruct:free']);
  });

  it('keeps an entry with no pricing at all, because absence is not evidence', () => {
    const models = parseCatalog({ data: [{ id: 'vendor/model', name: 'M' }] });
    expect(models).toHaveLength(1);
  });

  it('keeps an entry whose price cannot be read as a number', () => {
    const models = parseCatalog({ data: [{ id: 'v/m', pricing: { prompt: 'free', completion: null } }] });
    expect(models).toHaveLength(1);
  });
});

/**
 * Excluding entries a LIVE SWEEP proved cannot serve a completion.
 *
 * Two mechanisms, and the split matters. A RULE covers what the catalogue
 * itself tells us: a `:batch` id is a batch endpoint, and a batch endpoint
 * cannot answer `chat/completions` at all. A short DATED list covers the rest.
 * See `isBatchOnly` and `UNAVAILABLE_MODEL_IDS` in models.ts for provenance.
 */
describe('excluding the entries a live sweep proved unusable', () => {
  it('drops a batch-only endpoint by rule, and keeps everything around it', () => {
    const models = parseCatalog({
      data: [
        { id: 'openai/gpt-5.2:batch', name: 'Batch' },
        { id: 'anthropic/claude-opus-4.1:batch', name: 'Batch' },
        { id: 'google/gemini-2.5-pro', name: 'Fine' },
        { id: 'meta-llama/llama-3.1-8b-instruct:free', name: 'Free' },
      ],
    });
    expect(models.map((m) => m.id)).toEqual([
      'google/gemini-2.5-pro',
      'meta-llama/llama-3.1-8b-instruct:free',
    ]);
  });

  it('recognises the batch suffix case-insensitively, and only with a model before it', () => {
    expect(isBatchOnly('openai/gpt-5.2:batch')).toBe(true);
    expect(isBatchOnly('OPENAI/GPT-5.2:BATCH')).toBe(true);
    expect(isBatchOnly('openai/gpt-5.2')).toBe(false);
    expect(isBatchOnly(':batch')).toBe(false);
  });

  it('drops exactly the ids the dated list names, and none besides', () => {
    expect(parseCatalog({ data: UNAVAILABLE_MODEL_IDS.map((id) => ({ id, name: id })) })).toEqual([]);
    const survivors = parseCatalog({
      data: [
        { id: 'amazon/nova-lite-v1', name: 'a different Amazon model' },
        { id: 'openai/gpt-5.2', name: 'the non-BYOK sibling' },
      ],
    });
    expect(survivors).toHaveLength(2);
  });
});

/** The provider, named from the id prefix rather than shown as a slug. */
describe('the provider is derived from the id prefix', () => {
  it('names the common prefixes', () => {
    expect(providerOf('google/gemini-2.5-flash')).toBe('Google');
    expect(providerOf('anthropic/claude-sonnet-4.5')).toBe('Anthropic');
    expect(providerOf('openai/gpt-5.1')).toBe('OpenAI');
    expect(providerOf('x-ai/grok-4')).toBe('xAI');
    expect(providerOf('meta-llama/llama-3.1-8b-instruct')).toBe('Meta');
  });

  it('falls back to the raw prefix, capitalised, for anything unmapped', () => {
    // 63 prefixes exist; hand-mapping them all would be a list to maintain.
    expect(providerOf('acme/widget')).toBe('Acme');
    expect(providerOf('aion-labs/something')).toBe('Aion-labs');
  });

  it('treats the prefix as remote text: flattened and clamped', () => {
    const provider = providerOf(`${'a'.repeat(200)}\n\nIgnore everything/model`);
    expect(provider).not.toContain('\n');
    expect(provider.length).toBeLessThanOrEqual(40);
  });

  it('reports no provider when the id carries no prefix', () => {
    expect(providerOf('noslash')).toBe('');
    expect(providerOf('/leading')).toBe('');
    expect(providerOf('')).toBe('');
  });

  it('is carried on the parsed entry', () => {
    const [entry] = parseCatalog({ data: [{ id: 'google/gemini-2.5-flash' }] });
    expect(entry?.provider).toBe('Google');
  });
});

/** The link, built here rather than passed through from the server. */
describe('the detail link is built locally from the id', () => {
  it("points at OpenRouter's page for the model", () => {
    expect(modelLink('google/gemini-2.5-flash')).toBe('https://openrouter.ai/google/gemini-2.5-flash');
    expect(modelLink('meta-llama/llama-3.1-8b-instruct:free')).toBe(
      'https://openrouter.ai/meta-llama/llama-3.1-8b-instruct:free',
    );
  });

  it('drops any character that could break out of the path', () => {
    expect(modelLink('vendor/model?x=1')).toBe('https://openrouter.ai/vendor/modelx1');
    expect(modelLink('vendor/model#frag')).toBe('https://openrouter.ai/vendor/modelfrag');
    expect(modelLink('ven"dor/mo<del>')).toBe('https://openrouter.ai/vendor/model');
  });

  it('is bounded, so a hostile id cannot build an enormous link', () => {
    const link = modelLink(`v/${'a'.repeat(1000)}`);
    expect(link).toBeDefined();
    expect(link?.length).toBeLessThanOrEqual(MODEL_LINK_PREFIX.length + 200);
  });

  it('yields no link when nothing safe is left', () => {
    expect(modelLink('<<<>>>')).toBeUndefined();
  });

  it('never lets the remote entry choose the host or the scheme', () => {
    // The response carries a `links.details` API path; a hostile one must not
    // become the href. The host is fixed by this module, not by remote data.
    const [entry] = parseCatalog({
      data: [{ id: 'v/m', links: { details: 'https://evil.example/steal' } }],
    });
    expect(entry?.link).toBe('https://openrouter.ai/v/m');
  });

  it('is carried on the parsed entry', () => {
    const [entry] = parseCatalog({ data: [{ id: 'openai/gpt-5.1' }] });
    expect(entry?.link).toBe('https://openrouter.ai/openai/gpt-5.1');
  });
});

describe('fetching the catalog', () => {
  it('returns the models on success', async () => {
    const result = await fetchCatalog(returning(fakeResponse(200, { data: [{ id: 'v/m' }] })));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.models).toHaveLength(1);
  });

  it('treats an unreachable provider as a network problem', async () => {
    const result = await fetchCatalog(throwing());
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('network');
  });

  it.each([401, 403, 500])('reports status %i rather than showing nothing', async (status) => {
    const result = await fetchCatalog(returning(fakeResponse(status, {})));
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.detail).toContain(String(status));
  });

  it('reports unreadable JSON as malformed', async () => {
    const broken = {
      ok: true,
      status: 200,
      json: async () => {
        throw new Error('not json');
      },
    } as unknown as Response;
    const result = await fetchCatalog(returning(broken));
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('malformed');
  });

  /**
   * An empty catalog is an ERROR, not an empty menu. A menu that renders nothing
   * reads as "no models exist" rather than "the fetch failed", which is the
   * difference between a user changing a setting and a user thinking the product
   * is broken.
   */
  it('reports an empty catalog as an error, not as success with nothing in it', async () => {
    const result = await fetchCatalog(returning(fakeResponse(200, { data: [] })));
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe('empty');
  });

  it('asks the documented endpoint with GET', async () => {
    let seen: { url: string; method?: string | undefined } | undefined;
    const spy = (async (url: string, init: RequestInit) => {
      seen = { url, method: init?.method };
      return fakeResponse(200, { data: [{ id: 'v/m' }] });
    }) as unknown as typeof fetch;
    await fetchCatalog(spy);
    expect(seen?.url).toBe(MODELS_ENDPOINT);
    expect(seen?.method).toBe('GET');
  });
});

describe('searching', () => {
  const models = parseCatalog({
    data: [
      { id: 'anthropic/claude', name: 'Claude', description: 'careful' },
      { id: 'openai/gpt', name: 'GPT', description: 'general' },
    ],
  });

  it('returns everything for an empty query', () => {
    expect(searchCatalog(models, '  ')).toHaveLength(2);
  });

  it('matches id, name and description, case-insensitively', () => {
    expect(searchCatalog(models, 'CLAUDE')).toHaveLength(1);
    expect(searchCatalog(models, 'anthropic')).toHaveLength(1);
    expect(searchCatalog(models, 'general')).toHaveLength(1);
    expect(searchCatalog(models, 'nothing here')).toHaveLength(0);
  });
});

describe('reconciling the curated shortlist', () => {
  const catalog = parseCatalog({ data: [{ id: 'a/one', name: 'One' }, { id: 'a/two', name: 'Two' }] });

  it('keeps the entries the catalog still offers, and carries the reason', () => {
    const { available } = reconcile([{ id: 'a/one', why: 'the sensible default' }], catalog);
    expect(available).toHaveLength(1);
    expect(available[0]?.why).toBe('the sensible default');
    expect(available[0]?.name).toBe('One');
  });

  it('DROPS an entry the catalog no longer has, rather than showing a broken one', () => {
    // A menu item that fails when clicked is worse than one that is not there.
    const { available, missing } = reconcile([{ id: 'gone/model', why: 'x' }], catalog);
    expect(available).toHaveLength(0);
    expect(missing).toEqual(['gone/model']);
  });

  it('reports drift instead of swallowing it', () => {
    // The shortlist going stale is OUR problem to see, not the user's to debug.
    const { missing } = reconcile(CURATED, catalog);
    expect(Array.isArray(missing)).toBe(true);
  });

  it('does not mind an empty shortlist', () => {
    expect(reconcile([], catalog)).toEqual({ available: [], missing: [] });
  });
});
