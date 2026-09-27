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
 */

/** How long a remote string may be before it stops being a label. */
const NAME_MAX = 80;
const DESCRIPTION_MAX = 240;

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
  /** Reported context window, or undefined when the server did not say. */
  contextLength?: number;
}

export type CatalogResult =
  | { ok: true; models: CatalogEntry[] }
  | { ok: false; reason: 'network' | 'malformed' | 'empty'; detail: string };

export const MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';

/** Turn one raw entry into a display-safe one, or reject it. */
function parseEntry(raw: unknown): CatalogEntry | undefined {
  if (raw === null || typeof raw !== 'object') return undefined;
  const entry = raw as Record<string, unknown>;

  const id = typeof entry.id === 'string' ? entry.id.trim() : '';
  if (id.length === 0) return undefined;

  const contextRaw = entry.context_length ?? entry.contextLength;
  const contextLength =
    typeof contextRaw === 'number' && Number.isFinite(contextRaw) && contextRaw > 0
      ? Math.floor(contextRaw)
      : undefined;

  const name = sanitiseRemoteText(entry.name, NAME_MAX) || sanitiseRemoteText(id, NAME_MAX);
  const description = sanitiseRemoteText(entry.description, DESCRIPTION_MAX);

  // NOTE what is NOT here: no tier, no trust level, no policy field, no capability
  // flags. A catalog entry describes what a model IS, never what the gate should
  // do about it.
  return { id, name, description, ...(contextLength ? { contextLength } : {}) };
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
