// Fails the build when the CSS uses a custom property that was never defined.
//
// WHY: the semantic-token bug. `tokens.json` referenced its sources with paths
// missing their root, so fifteen semantic tokens were silently skipped. The
// stylesheet built fine. The card rendered with no coral, no filled button and
// no borders — structurally correct and visually broken, which nobody notices
// until they look at it.
//
// A missing CSS variable is not an error to the browser. It is `initial`, so the
// declaration is simply dropped. That makes it exactly the kind of failure this
// project keeps hitting: silent, and nearly right. So it gets a build-time check.
//
// Usage: node scripts/check-tokens.mjs

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = dirname(here);

/** Directories worth scanning. Kept explicit so this never wanders into node_modules. */
const ROOTS = [
  join(repoRoot, 'apps', 'desktop', 'web'),
  join(repoRoot, 'design'),
];

const SCAN_EXTENSIONS = new Set(['.css', '.js', '.mjs', '.html']);

function walk(dir, files = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return files; // a missing directory is not this check's problem
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, files);
    else if (SCAN_EXTENSIONS.has(extname(entry))) files.push(full);
  }
  return files;
}

const files = ROOTS.flatMap((root) => walk(root));

// ---------------------------------------------------------------------------
// Definitions: every `--name:` declared anywhere, including per-component ones
// like `--agent-accent` that live in app.css rather than in the token file.
// ---------------------------------------------------------------------------
const defined = new Set();
const DEFINITION = /(--[a-zA-Z0-9-]+)\s*:/g;

// ---------------------------------------------------------------------------
// Usages: every `var(--name)` reference.
// ---------------------------------------------------------------------------
const usages = [];
const USAGE = /var\(\s*(--[a-zA-Z0-9-]+)/g;

for (const file of files) {
  const text = readFileSync(file, 'utf8');
  const label = relative(repoRoot, file);

  for (const match of text.matchAll(DEFINITION)) defined.add(match[1]);

  for (const match of text.matchAll(USAGE)) {
    // Approximate line number, good enough to go and look.
    const line = text.slice(0, match.index).split('\n').length;
    usages.push({ file: label, line, name: match[1] });
  }
}

if (usages.length === 0) {
  console.error('check-tokens: found no var() usages at all — that is suspicious, not a pass.');
  console.error('If the frontend moved, update ROOTS in scripts/check-tokens.mjs.');
  process.exit(1);
}

const missing = usages.filter((u) => !defined.has(u.name));

if (missing.length > 0) {
  console.error(`check-tokens: FAILED — ${missing.length} custom propert${missing.length === 1 ? 'y is' : 'ies are'} used but never defined.\n`);
  const unique = [...new Set(missing.map((m) => m.name))];
  for (const name of unique) {
    console.error(`  ${name}`);
    for (const m of missing.filter((x) => x.name === name)) {
      console.error(`      used at ${m.file}:${m.line}`);
    }
  }
  console.error('\nA browser drops these declarations silently, so the UI degrades and looks nearly right.');
  console.error('Either define the token, or stop referencing it.');
  process.exit(1);
}

console.log(
  `check-tokens: ok — ${usages.length} var() references across ${files.length} files, ` +
    `all ${defined.size} defined properties resolve.`,
);
