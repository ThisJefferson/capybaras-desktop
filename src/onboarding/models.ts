/**
 * The model catalog.
 *
 * Scope decision: **only models reachable through the OpenRouter OAuth key are
 * listed.** No other provider, and therefore **no pasted API keys anywhere in the
 * product.** That removes an entire class of problem — the settings menu cannot
 * become a way to write some other provider's credential to disk, because there
 * is no field to put one in.
 *
 * TWO THINGS THIS MODULE IS CAREFUL ABOUT.
 *
 * 1. **The catalog is REMOTE DATA.** Model names and descriptions come from a
 *    server, not from us. They are rendered in our interface, which makes them
 *    attacker-influenced text in a trusted position — the same shape as the
 *    approval card (docs/threat-model.md, T4), and covered by the same
 *    `textContent`-only rule (T3). Everything that reaches the screen is
 *    sanitised and clamped first.
 *
 * 2. **A model choice must never affect the gate.** There is a tempting design
 *    where "fast/cheap model" quietly comes with lighter checks. Model choice
 *    and action risk are separate axes: a cheaper model still gets a hard gate on
 *    a production delete. This module therefore returns **display fields only** —
 *    there is nothing in its output the policy could consume, and a test asserts
 *    exactly that.
 *
 * 3. **The provider and the detail link are DERIVED here, not passed through.**
 *    `top_provider` carries no name, and `links.details` is a relative API path,
 *    so neither can be shown as-is. The provider comes from the id prefix, and the
 *    link is built locally as `https://openrouter.ai/<id>` — never a URL the
 *    server sent. The id is remote data, so it is charset-restricted and
 *    length-bounded before it becomes part of a URL.
 */

/** How long a remote string may be before it stops being a label. */
const NAME_MAX = 80;
const DESCRIPTION_MAX = 240;
/** How long a provider label may be. Derived from the id prefix, so remote. */
const PROVIDER_MAX = 40;
/** The longest id a link will carry. A link is convenience, not data. */
const LINK_ID_MAX = 200;
/** Exactly the characters an OpenRouter id uses. Anything else is dropped. */
const LINK_ID_DISALLOWED = /[^A-Za-z0-9._~:@/-]/g;
/** The only host a detail link may point at. Fixed locally, never remote. */
export const MODEL_LINK_PREFIX = 'https://openrouter.ai/';

/**
 * Flatten and clamp a string that came from somewhere else.
 *
 * Control characters become spaces so a remote value cannot forge layout, and
 * the length is bounded so it cannot bury the interface around it. The same
 * treatment the approval card's target label gets, for the same reason.
 */
export function sanitiseRemoteText(value: unknown, max: number): string {
  if (typeof value !== 'string') return '';
  const flattened = value
    .replace(/[\u0000-\u001F\u007F-\u009F]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return flattened.length <= max ? flattened : `${flattened.slice(0, max - 1)}\u2026`;
}

export interface CatalogEntry {
  /** The identifier sent to the API. Kept verbatim: it is an opaque key, not
   *  display text, and altering it would break the request. */
  id: string;
  /** Human-facing, sanitised. */
  name: string;
  description: string;
  /** Which API serves this model, derived from the id prefix. Display only. */
  provider: string;
  /** OpenRouter's page for this model, built locally. Absent when the id cannot
   *  safely form one. Display only — the interface opens it, nothing consumes it. */
  link?: string;
  /** Reported context window, or undefined when the server did not say. */
  contextLength?: number;
}

export type CatalogResult =
  | { ok: true; models: CatalogEntry[] }
  | { ok: false; reason: 'network' | 'malformed' | 'empty'; detail: string };

export const MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';

/**
 * Provider labels for the prefixes worth naming, so a person sees "Google"
 * rather than "google". Deliberately small: 63 prefixes exist and hand-mapping
 * all of them would be a list to maintain against a catalogue that churns. The
 * fallback below is the point — an unmapped prefix still shows, capitalised.
 */
const PROVIDER_LABELS: Readonly<Record<string, string>> = Object.freeze({
  anthropic: 'Anthropic',
  amazon: 'Amazon',
  cohere: 'Cohere',
  deepseek: 'DeepSeek',
  google: 'Google',
  groq: 'Groq',
  'meta-llama': 'Meta',
  microsoft: 'Microsoft',
  mistralai: 'Mistral',
  moonshotai: 'Moonshot AI',
  nvidia: 'NVIDIA',
  openai: 'OpenAI',
  perplexity: 'Perplexity',
  qwen: 'Qwen',
  'x-ai': 'xAI',
  'z-ai': 'Z.AI',
});

/**
 * The API that serves a model, from its id prefix.
 *
 * `google/gemini-2.5-flash` -> "Google". `top_provider` has no name field, so
 * the prefix is the only honest source. The prefix is REMOTE DATA rendered in the
 * interface, so an unmapped one is flattened and clamped like any other label.
 */
export function providerOf(id: string): string {
  const slash = id.indexOf('/');
  const prefix = slash > 0 ? id.slice(0, slash) : '';
  if (prefix.length === 0) return '';
  const mapped = PROVIDER_LABELS[prefix.toLowerCase()];
  if (mapped !== undefined) return mapped;
  // Not one we know: show the raw prefix, sanitised, with its first letter up so
  // a bare "acme" does not read as a typo rather than a name.
  const safe = sanitiseRemoteText(prefix, PROVIDER_MAX);
  if (safe.length === 0) return '';
  return safe.charAt(0).toUpperCase() + safe.slice(1);
}

/**
 * A link to OpenRouter's page for this model, BUILT LOCALLY.
 *
 * Not `links.details`, which is a relative API path, and never a URL the server
 * sent: a link is an injection surface, and the host is fixed here rather than
 * chosen by remote data. The id is charset-restricted to what an OpenRouter id
 * actually uses and length-bounded, then dropped entirely if nothing safe is
 * left, so a malformed id yields no link rather than a mangled one.
 */
export function modelLink(id: string): string | undefined {
  if (typeof id !== 'string') return undefined;
  const safe = id.replace(LINK_ID_DISALLOWED, '').slice(0, LINK_ID_MAX);
  if (safe.length === 0) return undefined;
  return `${MODEL_LINK_PREFIX}${safe}`;
}

/**
 * Whether an entry's price is NEGATIVE.
 *
 * This is not quality filtering. OpenRouter publishes `-1` pricing for its own
 * routers and meta entries (`openrouter/auto`, `openrouter/fusion`, ...), which
 * dispatch to other models rather than serving a completion — they cannot be
 * called as written. Six of the 458 entries measured on 2026-09-28 carried
 * negative pricing and every one was such a router. The rule is deliberately
 * narrow: only a negative figure disqualifies. A free model prices at 0, so it
 * stays, and a value that cannot be read as a number is not evidence either way.
 */
function hasNegativePrice(entry: Record<string, unknown>): boolean {
  const pricing = entry.pricing;
  if (pricing === null || typeof pricing !== 'object') return false;
  const fields = pricing as Record<string, unknown>;
  return [fields.prompt, fields.completion].some((raw) => {
    const value = typeof raw === 'number' ? raw : typeof raw === 'string' ? Number(raw) : NaN;
    return Number.isFinite(value) && value < 0;
  });
}

/** Turn one raw entry into a display-safe one, or reject it. */
function parseEntry(raw: unknown): CatalogEntry | undefined {
  if (raw === null || typeof raw !== 'object') return undefined;
  const entry = raw as Record<string, unknown>;

  const id = typeof entry.id === 'string' ? entry.id.trim() : '';
  if (id.length === 0) return undefined;

  // A router or meta entry cannot serve a completion. Drop it, narrowly.
  if (hasNegativePrice(entry)) return undefined;

  const contextRaw = entry.context_length ?? entry.contextLength;
  const contextLength =
    typeof contextRaw === 'number' && Number.isFinite(contextRaw) && contextRaw > 0
      ? Math.floor(contextRaw)
      : undefined;

  const name = sanitiseRemoteText(entry.name, NAME_MAX) || sanitiseRemoteText(id, NAME_MAX);
  const description = sanitiseRemoteText(entry.description, DESCRIPTION_MAX);
  const provider = providerOf(id);
  const link = modelLink(id);

  // NOTE what is NOT here: no tier, no trust level, no policy field, no capability
  // flags. A catalog entry describes what a model IS, never what the gate should
  // do about it. `provider` and `link` are the same kind of thing: what the model
  // is, and where to read about it.
  return {
    id,
    name,
    description,
    provider,
    ...(link ? { link } : {}),
    ...(contextLength ? { contextLength } : {}),
  };
}

/** Parse a `/models` response. Tolerant of extra fields, strict about shape. */
export function parseCatalog(raw: unknown): CatalogEntry[] {
  const list = (raw as { data?: unknown } | null)?.data;
  if (!Array.isArray(list)) return [];
  return list.map(parseEntry).filter((entry): entry is CatalogEntry => entry !== undefined);
}

export async function fetchCatalog(fetchImpl: typeof fetch): Promise<CatalogResult> {
  let response: Response;
  try {
    response = await fetchImpl(MODELS_ENDPOINT, { method: 'GET' });
  } catch {
    // Deliberately does not interpolate the error: a fetch failure can carry the
    // request, and while this request has no credential in it, the habit is the
    // point. See flow.ts for the case where it does matter.
    return { ok: false, reason: 'network', detail: 'could not reach OpenRouter to list models' };
  }

  if (!response.ok) {
    return { ok: false, reason: 'malformed', detail: `OpenRouter returned an unexpected status (${response.status})` };
  }

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return { ok: false, reason: 'malformed', detail: 'the model list could not be read' };
  }

  const models = parseCatalog(body);
  if (models.length === 0) {
    // An empty catalog is an error, not an empty menu. A menu that renders
    // nothing looks like "no models exist" rather than "the fetch failed".
    return { ok: false, reason: 'empty', detail: 'OpenRouter returned no usable models' };
  }
  return { ok: true, models };
}

/** Case-insensitive substring match over id, name and description. */
export function searchCatalog(models: readonly CatalogEntry[], query: string): CatalogEntry[] {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) return [...models];
  return models.filter((model) =>
    `${model.id} ${model.name} ${model.description}`.toLowerCase().includes(needle),
  );
}

export interface CuratedModel {
  /** The model to prefer. */
  id: string;
  /** Why someone who does not know these names should pick it. */
  why: string;
}

/**
 * A SHORT list, deliberately. OpenRouter carries hundreds of models, and a flat
 * list of hundreds is the same failure as approval fatigue: too much choice
 * produces either paralysis or a random pick.
 *
 * These IDs are a STARTING POINT and **must be reconciled against the live
 * catalog** — see `reconcile`, which drops anything missing rather than showing a
 * broken entry. Model names churn; the code should not break when they do.
 */
export const CURATED: readonly CuratedModel[] = Object.freeze([
  { id: 'anthropic/claude-sonnet-4.5', why: 'a good all-rounder, and careful with instructions' },
  { id: 'openai/gpt-5.1', why: 'strong general ability' },
  { id: 'google/gemini-2.5-pro', why: 'large context — good for long documents' },
  { id: 'deepseek/deepseek-chat-v3.1', why: 'much cheaper, still capable' },
]);

export interface Reconciled {
  /** Curated entries the catalog actually offers. */
  available: Array<CatalogEntry & { why: string }>;
  /** Curated IDs the catalog does not have — dropped, and worth reporting. */
  missing: string[];
}

/**
 * Match the curated shortlist against the live catalog.
 *
 * Missing entries are DROPPED rather than shown, because a menu item that fails
 * when clicked is worse than an item that is not there. `missing` is returned so
 * the caller can report the drift instead of swallowing it — a stale shortlist
 * should be visible to us, not confusing to the user.
 */
export function reconcile(curated: readonly CuratedModel[], catalog: readonly CatalogEntry[]): Reconciled {
  const byId = new Map(catalog.map((entry) => [entry.id, entry]));
  const available: Array<CatalogEntry & { why: string }> = [];
  const missing: string[] = [];

  for (const pick of curated) {
    const entry = byId.get(pick.id);
    if (entry) available.push({ ...entry, why: pick.why });
    else missing.push(pick.id);
  }

  return { available, missing };
}
