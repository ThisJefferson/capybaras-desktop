/**
 * Persistence for the grant store.
 *
 * The store itself stays pure and in-memory. This module is the only place that
 * touches the filesystem, and it follows two rules:
 *
 *   1. **Fail closed.** A missing, unreadable or corrupt file yields an EMPTY
 *      store — which means everything asks again. That is the safe direction.
 *      A persistence bug must never be able to *grant* authority.
 *   2. **Never throw.** A bad grants file must not stop the agent from starting.
 *      It is reported and ignored.
 *
 * Writes go through a temporary file and a rename, so a crash mid-write cannot
 * leave a half-written grants file behind.
 */

import { mkdirSync, readFileSync } from 'node:fs';

import { writeAtomic } from '../atomic-write';
import { dirname } from 'node:path';

import { GrantStore } from './grants';

/** Bumped if the on-disk shape ever changes. An unknown format is ignored. */
const FORMAT = 1;

export interface LoadResult {
  store: GrantStore;
  /** How many grants were re-admitted. */
  loaded: number;
  /** How many entries were present but refused. */
  rejected: number;
  /** Set when the file existed but could not be used at all. */
  problem?: string;
}

/**
 * Read grants from disk. Never throws, never partially applies a corrupt file.
 */
export function loadGrants(path: string, now: number = Date.now()): LoadResult {
  const store = new GrantStore();
  const empty = (problem?: string, rejected = 0): LoadResult => ({
    store,
    loaded: 0,
    rejected,
    ...(problem ? { problem } : {}),
  });

  let raw: string;
  try {
    raw = readFileSync(path, 'utf8');
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    // No file is the normal first-run case, not a problem.
    if (code === 'ENOENT') return empty();
    return empty(`could not read grants (${code ?? 'unknown error'})`);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return empty('grants file is not valid JSON');
  }

  if (parsed === null || typeof parsed !== 'object') {
    return empty('grants file is not an object');
  }

  const file = parsed as { format?: unknown; grants?: unknown };
  if (file.format !== FORMAT) {
    return empty(`unsupported grants format (${String(file.format)})`);
  }
  if (!Array.isArray(file.grants)) {
    return empty('grants file has no grants array');
  }

  let loaded = 0;
  let rejected = 0;
  for (const candidate of file.grants) {
    // `restore` applies the same rules as a fresh approval.
    if (store.restore(candidate, now)) loaded += 1;
    else rejected += 1;
  }

  return { store, loaded, rejected };
}

/**
 * Write grants to disk atomically. Returns an error message on failure rather
 * than throwing — being unable to save is worth reporting, not worth crashing
 * the agent over.
 */
export function saveGrants(path: string, store: GrantStore, now: number = Date.now()): string | undefined {
  const payload = JSON.stringify(
    {
      format: FORMAT,
      savedAt: new Date(now).toISOString(),
      grants: store.serialize(now),
    },
    null,
    2,
  );

  try {
    mkdirSync(dirname(path), { recursive: true });
    // Temp-and-rename lives in one place now (`src/atomic-write`) rather than
    // being reimplemented per file. This call gains two things the local version
    // did not have: a UNIQUE temporary name -- the old fixed `${path}.tmp` could
    // collide if two saves overlapped -- and an fsync, so the content is durable
    // before the rename that makes it visible.
    const written = writeAtomic(path, payload);
    if (!written.ok) return `could not save grants: ${written.detail}`;
    return undefined;
  } catch (error) {
    return `could not save grants: ${(error as Error).message}`;
  }
}
