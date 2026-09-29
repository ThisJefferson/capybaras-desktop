import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

/**
 * Frontend safety invariants.
 *
 * WHY THIS FILE EXISTS. Capybaras renders text an ATTACKER WROTE — the approval
 * card shows a target label, and that label comes from the action. An XSS in this
 * window does not just deface a page; from the webview it can call the IPC layer,
 * which means it can answer an approval. The frontend is therefore a
 * single point of failure for the whole product (docs/threat-model.md, T3, T4).
 *
 * These are the two rules that keep that from being true. They are tests rather
 * than guidelines because a guideline is remembered until the day it is not.
 */

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = dirname(here);
const WEB = join(repoRoot, 'apps', 'desktop', 'web');

function filesIn(dir: string, extensions: Set<string>, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) filesIn(full, extensions, out);
    else if (extensions.has(extname(entry))) out.push(full);
  }
  return out;
}

describe('no XSS sinks in the frontend', () => {
  // Every one of these turns a STRING into HTML or CODE. With attacker-controlled
  // text on screen, any of them is a route from "a nasty filename" to "the card
  // approved itself".
  const FORBIDDEN: ReadonlyArray<{ pattern: RegExp; why: string }> = [
    { pattern: /\binnerHTML\b/, why: 'parses a string as HTML' },
    { pattern: /\bouterHTML\b/, why: 'parses a string as HTML' },
    { pattern: /\binsertAdjacentHTML\b/, why: 'parses a string as HTML' },
    { pattern: /\bdocument\.write\b/, why: 'parses a string as HTML' },
    { pattern: /\beval\s*\(/, why: 'executes a string as code' },
    { pattern: /\bnew\s+Function\s*\(/, why: 'executes a string as code' },
    { pattern: /setTimeout\s*\(\s*['"`]/, why: 'executes a string as code' },
  ];

  const jsFiles = filesIn(WEB, new Set(['.js', '.mjs']));

  it('finds the frontend files at all', () => {
    // If this fails the paths moved and every assertion below would pass
    // vacuously -- the "empty result read as a pass" mistake, in test form.
    expect(jsFiles.length).toBeGreaterThan(0);
  });

  it.each(FORBIDDEN)('never uses $pattern ($why)', ({ pattern }) => {
    const offenders: string[] = [];
    for (const file of jsFiles) {
      const text = readFileSync(file, 'utf8');
      const lines = text.split('\n');
      lines.forEach((line, index) => {
        if (pattern.test(line)) offenders.push(`${file.replace(repoRoot, '')}:${index + 1}: ${line.trim()}`);
      });
    }
    expect(offenders).toEqual([]);
  });
});

describe('the page cannot run inline code', () => {
  const html = readFileSync(join(WEB, 'index.html'), 'utf8');

  it('has no inline <script> block', () => {
    // CSP is `script-src 'self'`, so an inline block would be BLOCKED at runtime --
    // silently, in a way that looks like the script simply did not run.
    const inline = /<script(?![^>]*\bsrc=)[^>]*>[\s\S]*?<\/script>/i.test(html);
    expect(inline).toBe(false);
  });

  it('has no inline style block or style attribute', () => {
    // Same story for `default-src 'self'`: inline styles are blocked, so a style
    // attribute is a bug that hides as "the element is unstyled".
    const styleBlock = /<style[^>]*>[\s\S]*?<\/style>/i.test(html);
    const styleAttr = /\sstyle\s*=\s*["']/i.test(html);
    expect(styleBlock || styleAttr).toBe(false);
  });

  it('loads the frontend as external files', () => {
    expect(html).toMatch(/<link[^>]+href="[^"]*\.css"/i);
    expect(html).toMatch(/<script[^>]+src="[^"]*\.js"/i);
  });
});

describe('the shell sets a content security policy', () => {
  const conf = readFileSync(join(repoRoot, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'), 'utf8');

  it('declares a CSP rather than leaving it null', () => {
    // Tauri only enables CSP protection if it is set. `"csp": null` means no
    // protection, and it is a common way to "fix" a blocked script.
    expect(conf).not.toMatch(/"csp"\s*:\s*null/);
    expect(conf).toMatch(/"csp"\s*:\s*"[^"]+"/);
  });

  it('does not allow remote script origins', () => {
    // PARSED, NOT PATTERN-MATCHED. A JSON string may legitimately escape its own
    // characters -- `\u0027` IS an apostrophe -- so reading the file as text
    // reports the escaping rather than the value, and a CSP that is perfectly
    // correct fails the check written to protect it. That is not hypothetical: it
    // happened, and it reddened CI for a day. This reads what Tauri reads.
    const csp = JSON.parse(conf)?.app?.security?.csp ?? '';
    expect(csp).toMatch(/script-src[^;]*'self'/);
    expect(csp).not.toMatch(/script-src[^;]*https?:/);
    expect(csp).not.toMatch(/unsafe-inline/);
    expect(csp).not.toMatch(/unsafe-eval/);
  });
});
