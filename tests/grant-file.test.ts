import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { DEFAULT_TTL_MS, GrantStore, loadGrants, saveGrants } from '../src/policy/index';
import type { Classification } from '../src/risk-classifier/index';

const created: string[] = [];

function tempPath(name = 'grants.json'): string {
  const dir = mkdtempSync(join(tmpdir(), 'capybaras-grants-'));
  created.push(dir);
  return join(dir, name);
}

afterEach(() => {
  for (const dir of created.splice(0)) rmSync(dir, { recursive: true, force: true });
});

/** A classification that is permitted to be remembered. */
function rememberable(): Classification {
  return {
    tier: 'confirm',
    reasons: ['test'],
    allowRemember: true,
    requiresTypedConfirmation: false,
    blockedPatterns: [],
  };
}

function storeWithGrant(now: number): GrantStore {
  const store = new GrantStore();
  const result = store.record({
    scope: { tool: 'fs.write', target: 'notes.md' },
    classification: rememberable(),
    approvedBy: 'human',
    now,
  });
  if (!result.ok) throw new Error(`test setup failed: ${result.reason}`);
  return store;
}

describe('grant persistence', () => {
  it('round-trips a grant through the file', () => {
    const path = tempPath();
    const now = Date.now();
    saveGrants(path, storeWithGrant(now), now);

    const loaded = loadGrants(path, now);
    expect(loaded.problem).toBeUndefined();
    expect(loaded.loaded).toBe(1);
    expect(loaded.rejected).toBe(0);
    expect(loaded.store.find({ tool: 'fs.write', target: 'notes.md' }, now)).toBeDefined();
  });

  it('treats a missing file as a normal first run, not a problem', () => {
    const loaded = loadGrants(tempPath('does-not-exist.json'));
    expect(loaded.loaded).toBe(0);
    expect(loaded.problem).toBeUndefined();
  });

  it('creates the directory it needs', () => {
    const path = join(tempPath(), 'nested', 'deeper', 'grants.json');
    expect(saveGrants(path, storeWithGrant(Date.now()))).toBeUndefined();
    expect(loadGrants(path).loaded).toBe(1);
  });

  it('writes a format marker and never leaves a temp file behind', () => {
    const path = tempPath();
    saveGrants(path, storeWithGrant(Date.now()));
    const file = JSON.parse(readFileSync(path, 'utf8'));
    expect(file.format).toBe(1);
    expect(Array.isArray(file.grants)).toBe(true);
  });

  // ------------------------------------------------------------------
  // Failing closed. Each of these is a way persistence could have GRANTED
  // authority it should not have. The store must come back empty.
  // ------------------------------------------------------------------

  it('fails closed on a corrupt file', () => {
    const path = tempPath();
    writeFileSync(path, '{ this is not json', 'utf8');
    const loaded = loadGrants(path);
    expect(loaded.loaded).toBe(0);
    expect(loaded.problem).toMatch(/not valid JSON/i);
    expect(loaded.store.list()).toHaveLength(0);
  });

  it('fails closed on an unknown format version', () => {
    const path = tempPath();
    writeFileSync(path, JSON.stringify({ format: 99, grants: [] }), 'utf8');
    const loaded = loadGrants(path);
    expect(loaded.problem).toMatch(/unsupported grants format/i);
  });

  it('refuses to resurrect an expired grant', () => {
    const path = tempPath();
    const now = Date.now();
    saveGrants(path, storeWithGrant(now), now);

    // Read it back well after the TTL has passed.
    const later = now + DEFAULT_TTL_MS + 1;
    const loaded = loadGrants(path, later);
    expect(loaded.loaded).toBe(0);
    expect(loaded.rejected).toBe(1);
    expect(loaded.store.list(later)).toHaveLength(0);
  });

  it('refuses a wildcard target even if it is in the file', () => {
    const path = tempPath();
    const now = Date.now();
    writeFileSync(
      path,
      JSON.stringify({
        format: 1,
        grants: [
          {
            id: 'a', tool: 'fs.write', target: '*', grantedBy: 'human',
            createdAt: now, expiresAt: now + DEFAULT_TTL_MS,
          },
        ],
      }),
      'utf8',
    );
    const loaded = loadGrants(path, now);
    expect(loaded.rejected).toBe(1);
    expect(loaded.loaded).toBe(0);
  });

  it('refuses a grant whose provenance is not a human', () => {
    const path = tempPath();
    const now = Date.now();
    writeFileSync(
      path,
      JSON.stringify({
        format: 1,
        grants: [
          {
            id: 'a', tool: 'fs.write', target: 'notes.md', grantedBy: 'model',
            createdAt: now, expiresAt: now + DEFAULT_TTL_MS,
          },
        ],
      }),
      'utf8',
    );
    const loaded = loadGrants(path, now);
    expect(loaded.rejected).toBe(1);
    expect(loaded.loaded).toBe(0);
  });

  it('keeps the good entries when some entries are malformed', () => {
    const path = tempPath();
    const now = Date.now();
    writeFileSync(
      path,
      JSON.stringify({
        format: 1,
        grants: [
          { id: 'good', tool: 'fs.write', target: 'notes.md', grantedBy: 'human', createdAt: now, expiresAt: now + DEFAULT_TTL_MS },
          { nonsense: true },
          null,
          { id: 'bad', tool: 'fs.write', target: '', grantedBy: 'human', createdAt: now, expiresAt: now + DEFAULT_TTL_MS },
        ],
      }),
      'utf8',
    );
    const loaded = loadGrants(path, now);
    expect(loaded.loaded).toBe(1);
    expect(loaded.rejected).toBe(3);
  });

  it('does not persist expired grants when saving', () => {
    const path = tempPath();
    const now = Date.now();
    const store = new GrantStore();
    store.record({
      scope: { tool: 'fs.write', target: 'old.md' },
      classification: rememberable(),
      approvedBy: 'human',
      now: now - DEFAULT_TTL_MS - 1000, // already expired
    });
    saveGrants(path, store, now);
    const file = JSON.parse(readFileSync(path, 'utf8'));
    expect(file.grants).toHaveLength(0);
  });
});

describe('GrantStore.restore applies the same rules as record', () => {
  const now = Date.now();
  const valid = {
    id: 'x', tool: 'fs.write', target: 'notes.md', grantedBy: 'human',
    createdAt: now, expiresAt: now + DEFAULT_TTL_MS,
  };

  it('admits a well-formed grant', () => {
    const store = new GrantStore();
    expect(store.restore(valid, now)).toBe(true);
  });

  it.each([
    ['no id', { ...valid, id: '' }],
    ['no tool', { ...valid, tool: '' }],
    ['no target', { ...valid, target: '' }],
    ['wildcard target', { ...valid, target: '*' }],
    ['not human', { ...valid, grantedBy: 'system' }],
    ['non-numeric expiry', { ...valid, expiresAt: 'soon' }],
    ['already expired', { ...valid, expiresAt: now - 1 }],
    ['null', null],
  ])('refuses %s', (_label, candidate) => {
    expect(new GrantStore().restore(candidate, now)).toBe(false);
  });

  it('serialize prunes before returning', () => {
    const store = new GrantStore();
    store.restore({ ...valid, id: 'live', expiresAt: now + 1000 }, now);
    store.restore({ ...valid, id: 'dead', expiresAt: now - 1000 }, now);
    expect(store.serialize(now).map((g) => g.id)).toEqual(['live']);
  });
});
