import { describe, expect, it } from 'vitest';

import {
  MAX_FILE_BYTES,
  declaredKind,
  describeHazard,
  detectKind,
  inspect,
} from '../src/file-inspector';

/**
 * The structural-inspection corpus.
 *
 * Every case here is a claim about a file we would refuse to open, or about one
 * we would let through. The fixtures are built from bytes rather than shipped as
 * files, because a corpus of real hostile documents is a corpus we would have to
 * keep, scan, and explain. Bytes are honest, reviewable, and impossible to
 * mistake for something to run.
 *
 * The assertions are on `code`, never on the sentence. The sentences are for the
 * card and may be reworded; the codes are the contract.
 */

/* ------------------------------------------------------------------ */
/* Building bytes                                                      */
/* ------------------------------------------------------------------ */

const ascii = (s: string): number[] => Array.from(s, (c) => c.charCodeAt(0));

const bytes = (...parts: Array<string | number[] | Uint8Array>): Uint8Array => {
  const out: number[] = [];
  for (const p of parts) {
    if (typeof p === 'string') out.push(...ascii(p));
    else if (p instanceof Uint8Array) out.push(...Array.from(p));
    else out.push(...p);
  }
  return new Uint8Array(out);
};

const PDF = '%PDF-1.7\n';

/** One ZIP local-file-header carrying `name`, which is all this reader needs. */
const zipEntry = (name: string): number[] => {
  const fixed = new Array<number>(22).fill(0);
  const len = name.length;
  return [
    0x50, 0x4b, 0x03, 0x04,
    ...fixed,
    len & 0xff, (len >> 8) & 0xff, // name length
    0x00, 0x00, // extra length
    ...ascii(name),
  ];
};

const codes = (name: string, b: Uint8Array): string[] =>
  inspect(name, b).findings.map((f) => f.code);

/* ------------------------------------------------------------------ */
describe('what the bytes actually are', () => {
  it('reads the container signatures', () => {
    expect(detectKind(bytes(PDF))).toBe('pdf');
    expect(detectKind(new Uint8Array([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]))).toBe('ole');
    expect(detectKind(new Uint8Array([0x50, 0x4b, 0x03, 0x04]))).toBe('zip');
    expect(detectKind(bytes('{\\rtf1'))).toBe('rtf');
  });

  it('reads the executable signatures, on every platform we run on', () => {
    expect(detectKind(bytes('MZ'))).toBe('executable');
    expect(detectKind(new Uint8Array([0x7f, 0x45, 0x4c, 0x46]))).toBe('executable');
    expect(detectKind(new Uint8Array([0xfe, 0xed, 0xfa, 0xce]))).toBe('executable');
  });

  it('reads images, markup and scripts', () => {
    expect(detectKind(new Uint8Array([0xff, 0xd8, 0xff]))).toBe('image');
    expect(detectKind(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBe('image');
    expect(detectKind(bytes('GIF89a'))).toBe('image');
    expect(detectKind(bytes('<!DOCTYPE html><html>'))).toBe('html');
    expect(detectKind(bytes('#!/bin/sh\n'))).toBe('script');
    expect(detectKind(bytes('just some prose'))).toBe('text');
  });

  it('does not call a PDF prose just because it starts with ASCII', () => {
    // The container check must win over the text heuristic.
    expect(detectKind(bytes(PDF, 'anything'))).toBe('pdf');
  });

  it('says it does not know rather than guessing', () => {
    expect(detectKind(new Uint8Array([0x00, 0x01, 0x02, 0x03]))).toBe('unknown');
    expect(detectKind(new Uint8Array(0))).toBe('unknown');
  });
});

/* ------------------------------------------------------------------ */
describe('what the name claims', () => {
  it('maps the extensions that appear in real attacks', () => {
    expect(declaredKind('a.pdf')).toBe('pdf');
    expect(declaredKind('a.DOC')).toBe('ole');
    expect(declaredKind('a.docm')).toBe('zip');
    expect(declaredKind('a.exe')).toBe('executable');
    expect(declaredKind('a.ps1')).toBe('script');
    expect(declaredKind('a.PNG')).toBe('image');
  });

  it('treats an unknown or absent extension as no claim at all', () => {
    expect(declaredKind('a.qqq')).toBe('other');
    expect(declaredKind('noextension')).toBe('other');
    expect(declaredKind('trailing.')).toBe('other');
  });
});

/* ------------------------------------------------------------------ */
describe('the cheapest attack: a name that lies', () => {
  it('stops when a document is really a program', () => {
    const r = inspect('invoice.pdf', bytes('MZ', '\x90\x00'));
    expect(r.declared).toBe('pdf');
    expect(r.detected).toBe('executable');
    expect(r.mismatch).toBe(true);
    expect(r.hazard).toBe('stop');
    expect(r.findings.map((f) => f.code)).toContain('file.type_mismatch');
    expect(r.findings.map((f) => f.code)).toContain('file.executable');
  });

  it('does not cry mismatch when the name makes no claim', () => {
    const r = inspect('downloaded-file', bytes(PDF));
    expect(r.declared).toBe('other');
    expect(r.mismatch).toBe(false);
  });

  it('does not cry mismatch when the name is right', () => {
    const r = inspect('a.pdf', bytes(PDF));
    expect(r.mismatch).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
describe('a PDF that acts when it opens', () => {
  it('flags a script', () => {
    expect(codes('a.pdf', bytes(PDF, '/JavaScript (app.alert(1))'))).toContain('page.script');
  });

  it('flags an open action and a page event', () => {
    expect(codes('a.pdf', bytes(PDF, '/OpenAction 1 0 R'))).toContain('page.open_action');
    expect(codes('a.pdf', bytes(PDF, '/AA << /O 2 0 R >>'))).toContain('page.additional_action');
  });

  it('flags a launcher and an embedded file', () => {
    expect(codes('a.pdf', bytes(PDF, '/Launch /F (setup.exe)'))).toContain('page.launch');
    expect(codes('a.pdf', bytes(PDF, '/EmbeddedFile'))).toContain('page.embedded_file');
  });

  it('flags encryption, and says the contents are unknown rather than safe', () => {
    const r = inspect('a.pdf', bytes(PDF, '/Encrypt 3 0 R'));
    const enc = r.findings.find((f) => f.code === 'page.encrypted');
    expect(enc).toBeDefined();
    expect(enc?.because).toMatch(/unknown/i);
  });

  it('says nothing about an ordinary PDF', () => {
    const r = inspect('a.pdf', bytes(PDF, '1 0 obj << /Type /Catalog >> endobj'));
    expect(r.findings).toHaveLength(0);
    expect(r.hazard).toBe('none');
  });
});

/* ------------------------------------------------------------------ */
describe('a document carrying a program', () => {
  it('stops on a legacy macro', () => {
    const ole = new Uint8Array([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    const r = inspect('sheet.xls', bytes(ole, '_VBA_PROJECT'));
    expect(r.hazard).toBe('stop');
    expect(r.findings.map((f) => f.code)).toContain('office.macro');
  });

  it('stops on an OOXML macro part', () => {
    const z = byteZip(['[Content_Types].xml', 'word/vbaProject.bin']);
    const r = inspect('report.docm', z);
    expect(r.hazard).toBe('stop');
    expect(r.findings.map((f) => f.code)).toContain('office.macro');
  });

  it('stops on a program inside an archive', () => {
    const z = byteZip(['readme.txt', 'setup.exe']);
    expect(codes('bundle.zip', z)).toContain('archive.executable');
  });

  it('stops on entries that climb out of the archive', () => {
    const z = byteZip(['../../etc/passwd']);
    expect(codes('bundle.zip', z)).toContain('archive.absolute_paths');
  });

  it('flags an archive inside an archive', () => {
    const z = byteZip(['notes.txt', 'inner.zip']);
    expect(codes('bundle.zip', z)).toContain('archive.nested');
  });

  it('lets an ordinary archive through', () => {
    const z = byteZip(['notes.txt', 'data.csv']);
    const r = inspect('bundle.zip', z);
    expect(r.findings).toHaveLength(0);
    expect(r.hazard).toBe('none');
  });
});

/** Build an archive whose local file headers carry exactly these names. */
function byteZip(names: string[]): Uint8Array {
  const out: number[] = [];
  for (const n of names) out.push(...zipEntry(n));
  return new Uint8Array(out);
}

/* ------------------------------------------------------------------ */
describe('a program is never a document', () => {
  it('stops on an executable whatever it was called', () => {
    const r = inspect('picture.png', bytes('MZ', '\x00'));
    expect(r.hazard).toBe('stop');
    expect(r.findings.map((f) => f.code)).toContain('file.executable');
  });
});

/* ------------------------------------------------------------------ */
describe('bounds (T16)', () => {
  it('refuses to examine an enormous file, and says so', () => {
    const r = inspect('huge.bin', new Uint8Array(MAX_FILE_BYTES + 1));
    expect(r.oversized).toBe(true);
    expect(r.hazard).toBe('caution');
    expect(r.findings.map((f) => f.code)).toContain('file.oversized');
  });

  it('does not work harder than the entry cap', () => {
    // Far more headers than the cap; the reader must stop counting, not hang.
    const many: number[] = [];
    for (let i = 0; i < 6_000; i += 1) many.push(...zipEntry(`f${i}.txt`));
    const r = inspect('many.zip', new Uint8Array(many));
    expect(r.findings.map((f) => f.code)).toContain('archive.too_many_entries');
  });
});

/* ------------------------------------------------------------------ */
describe('reading it out loud (D23: once)', () => {
  it('says nothing when there is nothing to say', () => {
    expect(describeHazard(inspect('a.pdf', bytes(PDF)))).toBeNull();
  });

  it('gives one sentence, and counts the rest', () => {
    const r = inspect('a.pdf', bytes(PDF, '/JavaScript', '/Launch', '/OpenAction'));
    const said = describeHazard(r);
    expect(said).toBeTruthy();
    expect(said).toMatch(/and 2 more/);
  });

  it('leads with a stop-level finding, never a note', () => {
    const r = inspect('invoice.pdf', bytes('MZ', '\x00'));
    const said = describeHazard(r) ?? '';
    expect(said).toMatch(/named as one kind|program/i);
    expect(said).toMatch(/and 1 more/);
  });
});

/* ------------------------------------------------------------------ */
describe('it is pure', () => {
  it('does not modify the bytes it is given', () => {
    const input = bytes(PDF, '/JavaScript');
    const copy = Uint8Array.from(input);
    inspect('a.pdf', input);
    expect(Array.from(input)).toEqual(Array.from(copy));
  });

  it('gives the same answer twice', () => {
    const b = bytes(PDF, '/OpenAction');
    expect(inspect('a.pdf', b).findings).toEqual(inspect('a.pdf', b).findings);
  });
});
