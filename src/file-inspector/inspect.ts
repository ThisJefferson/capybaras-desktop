/**
 * Structural inspection — what a file IS, before anything opens it.
 *
 * (docs/threat-model.md, T13 and T16.)
 *
 * **THE THREAT.** A file arriving from a person, a download, or an attachment is
 * hostile until it is not. The dangerous thing is not usually the bytes a program
 * shows you — it is the payload carried alongside them: a PDF that launches a
 * script when it opens, a spreadsheet with a macro, a document whose extension
 * says `invoice.pdf` while its header says it is a Windows executable. None of
 * that is visible in a filename, and all of it is visible in the structure.
 *
 * **WHAT THIS DOES.** Reads the first bytes of a file and answers three questions
 * in plain language: *what is this really*, *does that match what it claims to
 * be*, and *does it carry anything that acts when opened*. It also enforces the
 * bounds in T16 — size, entry count, nesting — so a small file cannot become an
 * unbounded amount of work.
 *
 * **WHAT IT DELIBERATELY DOES NOT DO, stated so nobody assumes more.**
 *
 * 1. **It is not a virus scanner, and it never will be.** There is no signature
 *    feed, nothing to go stale, and no claim that a file is "clean". A file that
 *    passes this inspection is one whose *shape* is unremarkable — nothing more.
 *    The catalogue states this too: "we do not scan, and here is what we do
 *    instead".
 * 2. **It does not parse.** It reads structural markers and entry names. It never
 *    builds a document object, never executes anything, and never follows a
 *    reference. Parsing happens out of process, in the sidecar (T13).
 * 3. **It does not read the whole file.** Beyond the bounded head and a bounded
 *    scan window, the bytes are not examined. This is what keeps T16 closed.
 * 4. **It is not a gate.** It produces findings. The gate decides what to do with
 *    them, and the card says it once, in plain words (D23).
 *
 * **WHY IT IS PURE.** `inspect(name, bytes)` takes a name and bytes and returns
 * findings. No filesystem, no network, no clock. That makes every hazard here
 * testable without a hostile file, which is the only way this coverage is
 * maintainable.
 */

/** The bounded scan window. Beyond this the inspector does not look (T16). */
export const SCAN_WINDOW_BYTES = 4 * 1024 * 1024;

/** The largest file this inspector will consider at all (T16). */
export const MAX_FILE_BYTES = 512 * 1024 * 1024;

/** The most archive entries before we stop counting and say so (T16). */
export const MAX_ARCHIVE_ENTRIES = 5_000;

/** The deepest archive nesting we will describe before calling it a bomb (T16). */
export const MAX_ARCHIVE_DEPTH = 3;

/* ------------------------------------------------------------------ */
/* What a thing is                                                     */
/* ------------------------------------------------------------------ */

/** The shape the bytes have, as far as the head reveals. */
export type FileKind =
  | 'pdf'
  | 'ole' // legacy Office / compound document — where macros live
  | 'zip' // OOXML, ODF, and the archive itself
  | 'executable'
  | 'script'
  | 'image'
  | 'text'
  | 'html'
  | 'rtf'
  | 'unknown';

/**
 * What the person's extension *claims*. Only the families that appear in real
 * attacks matter here; everything else is `other` and is not a claim at all.
 */
export type DeclaredKind = FileKind | 'other';

/** How alarming a finding is, in the plainest possible terms. */
export type Hazard = 'none' | 'caution' | 'stop';

export interface Finding {
  /** Stable identifier. Tests assert on this, never on the sentence. */
  code: string;
  /** One plain sentence for the card. No jargon, no file-format names. */
  summary: string;
  /** Why it matters, in the same register. */
  because: string;
  hazard: Hazard;
}

export interface Inspection {
  /** What the name claims. */
  declared: DeclaredKind;
  /** What the bytes say. */
  detected: FileKind;
  /** True when those two disagree — the cheapest real attack there is. */
  mismatch: boolean;
  /** The file is larger than this inspector will examine. */
  oversized: boolean;
  findings: Finding[];
  /** The worst finding, reduced to one word, for the gate. */
  hazard: Hazard;
}

/* ------------------------------------------------------------------ */
/* Magic bytes                                                         */
/* ------------------------------------------------------------------ */

const startsWith = (bytes: Uint8Array, sig: readonly number[], at = 0): boolean => {
  if (bytes.length < at + sig.length) return false;
  for (let i = 0; i < sig.length; i += 1) {
    if (bytes[at + i] !== sig[i]) return false;
  }
  return true;
};

/** Read a little-endian ASCII run, for the text-ish formats. */
const headAscii = (bytes: Uint8Array, max: number): string => {
  const end = Math.min(bytes.length, max);
  let out = '';
  for (let i = 0; i < end; i += 1) {
    out += String.fromCharCode(bytes[i] ?? 0);
  }
  return out;
};

/**
 * What the first bytes say this is.
 *
 * Order matters: the container signatures are checked before the text
 * heuristics, because a PDF starts with ASCII that a naive text check would
 * happily call prose.
 */
export function detectKind(bytes: Uint8Array): FileKind {
  // Containers first — they are unambiguous.
  if (startsWith(bytes, [0x25, 0x50, 0x44, 0x46])) return 'pdf'; // %PDF
  if (startsWith(bytes, [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1])) return 'ole';
  if (startsWith(bytes, [0x50, 0x4b, 0x03, 0x04])) return 'zip'; // PK..
  if (startsWith(bytes, [0x50, 0x4b, 0x05, 0x06])) return 'zip'; // empty archive
  if (startsWith(bytes, [0x7b, 0x5c, 0x72, 0x74, 0x66])) return 'rtf'; // {\rtf

  // Executables. `MZ` covers PE; `\x7fELF` and the Mach-O magics cover the rest.
  if (startsWith(bytes, [0x4d, 0x5a])) return 'executable';
  if (startsWith(bytes, [0x7f, 0x45, 0x4c, 0x46])) return 'executable';
  if (
    startsWith(bytes, [0xfe, 0xed, 0xfa, 0xce]) ||
    startsWith(bytes, [0xfe, 0xed, 0xfa, 0xcf]) ||
    startsWith(bytes, [0xca, 0xfe, 0xba, 0xbe])
  ) {
    return 'executable';
  }

  // Script-ish shebang, before the text fallback.
  if (startsWith(bytes, [0x23, 0x21])) return 'script'; // #!

  // Images.
  if (startsWith(bytes, [0xff, 0xd8, 0xff])) return 'image'; // jpeg
  if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47])) return 'image'; // png
  if (startsWith(bytes, [0x47, 0x49, 0x46, 0x38])) return 'image'; // gif
  if (startsWith(bytes, [0x42, 0x4d])) return 'image'; // bmp

  // Text-ish. A UTF-8 BOM, then the markup check, then plain text.
  const head = headAscii(bytes, 512).toLowerCase();
  const body = startsWith(bytes, [0xef, 0xbb, 0xbf]) ? head.slice(1) : head;
  if (/^\s*<!doctype html|^\s*<html/.test(body)) return 'html';
  if (bytes.length > 0 && !/\u0000/.test(headAscii(bytes, 256))) return 'text';

  return 'unknown';
}

/** The kind a filename's extension claims, if it claims anything we know. */
export function declaredKind(name: string): DeclaredKind {
  const dot = name.lastIndexOf('.');
  if (dot < 0 || dot === name.length - 1) return 'other';
  const ext = name.slice(dot + 1).toLowerCase();
  switch (ext) {
    case 'pdf':
      return 'pdf';
    case 'doc':
    case 'xls':
    case 'ppt':
    case 'msi':
      return 'ole';
    case 'docm':
    case 'xlsm':
    case 'pptm':
    case 'docx':
    case 'xlsx':
    case 'pptx':
    case 'zip':
    case 'jar':
      return 'zip';
    case 'exe':
    case 'dll':
    case 'com':
    case 'scr':
    case 'bat':
    case 'cmd':
      return 'executable';
    case 'js':
    case 'mjs':
    case 'cjs':
    case 'vbs':
    case 'ps1':
    case 'sh':
    case 'py':
      return 'script';
    case 'jpg':
    case 'jpeg':
    case 'png':
    case 'gif':
    case 'bmp':
    case 'webp':
      return 'image';
    case 'txt':
    case 'md':
    case 'csv':
    case 'log':
      return 'text';
    case 'html':
    case 'htm':
      return 'html';
    case 'rtf':
      return 'rtf';
    default:
      return 'other';
  }
}

/* ------------------------------------------------------------------ */
/* Structural markers                                                  */
/* ------------------------------------------------------------------ */

/** Does the scan window contain this ASCII marker? Case-sensitive. */
const hasMarker = (window: string, marker: string): boolean => window.includes(marker);

/**
 * PDF: the markers that mean "this file does something when opened".
 *
 * These are the names the format uses, and they are read as text rather than
 * parsed — which is the point: it is cheap, it cannot execute anything, and it
 * does not need to understand the document to see a `/Launch`.
 */
function inspectPdf(window: string, findings: Finding[]): void {
  const actions: Array<[string, string, string, string]> = [
    [
      '/JavaScript',
      'page.script',
      'This document carries a script',
      'A script inside a document can run when the file is opened, not only when you ask it to.',
    ],
    [
      '/OpenAction',
      'page.open_action',
      'This document runs something when it opens',
      'Opening the file is the trigger. Nothing further is asked of you.',
    ],
    [
      '/AA',
      'page.additional_action',
      'This document runs something on a page event',
      'An automatic action fires on opening, closing, or printing the page.',
    ],
    [
      '/Launch',
      'page.launch',
      'This document tries to start another program',
      'A document that launches something else is how a reader becomes an installer.',
    ],
    [
      '/EmbeddedFile',
      'page.embedded_file',
      'This document carries another file inside it',
      'The inner file is not the one you were shown, and it has its own type.',
    ],
    [
      '/RichMedia',
      'page.rich_media',
      'This document embeds interactive media',
      'Embedded media has historically been used to reach a vulnerable reader.',
    ],
    [
      '/XFA',
      'page.xfa',
      'This document contains a form that can run logic',
      'XFA forms can carry script alongside what looks like a simple form.',
    ],
    [
      '/JBIG2Decode',
      'page.jbig2',
      'This document uses an image decoder with a history of bugs',
      'A memory-safety flaw in a decoder is reachable by opening the file.',
    ],
  ];
  for (const [marker, code, summary, because] of actions) {
    if (hasMarker(window, marker)) {
      findings.push({ code, summary, because, hazard: 'caution' });
    }
  }
  if (hasMarker(window, '/Encrypt')) {
    findings.push({
      code: 'page.encrypted',
      summary: 'This document is encrypted',
      because:
        'An encrypted document cannot be inspected as it is. Its contents are unknown until it is opened.',
      hazard: 'caution',
    });
  }
}

/**
 * Legacy Office (compound file): where macros live.
 *
 * A Word or Excel file from before the XML formats *is* a little filesystem, and
 * a macro is a program stored in it. The markers below are the storage names the
 * format uses for exactly that.
 */
function inspectOle(window: string, findings: Finding[]): void {
  const macroMarkers = ['VBA', '_VBA_PROJECT', 'Macros', 'vbaProject'];
  if (macroMarkers.some((m) => hasMarker(window, m))) {
    findings.push({
      code: 'office.macro',
      summary: 'This document contains a macro',
      because:
        'A macro is a program. It can run when the document is opened, and it can do anything the file could do.',
      hazard: 'stop',
    });
  }
}

/**
 * Office XML / ODF / archives: read the entry names, because the entry names are
 * the content types.
 *
 * This does not unzip anything. It reads the local file headers in place and
 * takes the names from them (T16: bounded, no expansion).
 */
function inspectZip(window: string, findings: Finding[]): void {
  // Local file header: PK\x03\x04, then a fixed 26 bytes, then name length,
  // then extra length, then the name itself.
  let at = 0;
  let entries = 0;
  let nested = 0;
  const names: string[] = [];
  while (entries < MAX_ARCHIVE_ENTRIES) {
    const next = window.indexOf('PK\u0003\u0004', at);
    if (next < 0 || next + 30 > window.length) break;
    const nameLen = window.charCodeAt(next + 26) | (window.charCodeAt(next + 27) << 8);
    const extraLen = window.charCodeAt(next + 28) | (window.charCodeAt(next + 29) << 8);
    const nameStart = next + 30;
    const name = window.slice(nameStart, nameStart + Math.min(nameLen, 512));
    if (name) names.push(name);
    if (/\.(zip|jar|docm|xlsm|pptm)$/i.test(name)) nested += 1;
    entries += 1;
    at = nameStart + nameLen + extraLen;
  }

  if (entries >= MAX_ARCHIVE_ENTRIES) {
    findings.push({
      code: 'archive.too_many_entries',
      summary: 'This archive holds an unusual number of items',
      because:
        'Counting them all would take longer than it is worth, and archives this size are not documents.',
      hazard: 'caution',
    });
  }
  if (nested > 0) {
    findings.push({
      code: 'archive.nested',
      summary: 'This file has another archive inside it',
      because:
        'Nesting is how one hidden layer hides the next. What is inside cannot be seen from here.',
      hazard: 'caution',
    });
  }

  const joined = names.join('\n');
  if (/vbaProject\.bin|\.bas\b|\.vba\b/i.test(joined)) {
    findings.push({
      code: 'office.macro',
      summary: 'This document contains a macro',
      because:
        'A macro is a program. It can run when the document is opened, and it can do anything the file could do.',
      hazard: 'stop',
    });
  }
  if (/\.(exe|dll|scr|com|js|vbs|ps1|bat|cmd|lnk)$/i.test(joined)) {
    findings.push({
      code: 'archive.executable',
      summary: 'This file contains a program',
      because:
        'An archive carrying something runnable is how a document becomes an installer.',
      hazard: 'stop',
    });
  }
  if (/(\/(|\\))?\.\.(\/|\\)/.test(joined) || /^[A-Za-z]:/.test(joined.trim())) {
    findings.push({
      code: 'archive.absolute_paths',
      summary: 'This archive writes to paths outside itself',
      because:
        'Entries that point upwards do not stay where they are unpacked.',
      hazard: 'stop',
    });
  }
  // Macro-enabled OOXML is a *type* marker, and worth stating on its own.
  if (names.some((n) => /word\/vbaProject|xl\/vbaProject|ppt\/vbaProject/i.test(n))) {
    findings.push({
      code: 'office.macro_part',
      summary: 'This document is a macro-enabled type',
      because:
        'The file itself declares that it carries macros, rather than merely being able to.',
      hazard: 'stop',
    });
  }
}

/* ------------------------------------------------------------------ */
/* The entry point                                                     */
/* ------------------------------------------------------------------ */

const worst = (findings: Finding[]): Hazard => {
  if (findings.some((f) => f.hazard === 'stop')) return 'stop';
  if (findings.some((f) => f.hazard === 'caution')) return 'caution';
  return 'none';
};

/**
 * Inspect a file by name and bytes. Pure: no filesystem, no network, no clock.
 *
 * The order of the work is deliberate. The size bound is applied first, so an
 * enormous file costs nothing. The mismatch check comes second, because it is
 * the cheapest real attack and the easiest thing for a person to understand. The
 * structural markers come last, and only for containers that have any.
 */
export function inspect(name: string, bytes: Uint8Array): Inspection {
  const declared = declaredKind(name);
  const findings: Finding[] = [];

  if (bytes.length > MAX_FILE_BYTES) {
    return {
      declared,
      detected: 'unknown',
      mismatch: false,
      oversized: true,
      findings: [
        {
          code: 'file.oversized',
          summary: 'This file is far larger than a document should be',
          because:
            'It is too large to inspect, so nothing about its contents can be described.',
          hazard: 'caution',
        },
      ],
      hazard: 'caution',
    };
  }

  const detected = detectKind(bytes);
  const mismatch = declared !== 'other' && declared !== detected;

  if (mismatch) {
    findings.push({
      code: 'file.type_mismatch',
      summary: `This is named as one kind of file but is another`,
      because:
        'The name is what you were shown. The bytes are what will actually be opened.',
      hazard: 'stop',
    });
  }

  // The markers are scanned in a bounded window, decoded as latin-1 so every
  // byte maps to exactly one character and no multi-byte sequence can slip a
  // marker past the check.
  const window = headAscii(bytes, Math.min(bytes.length, SCAN_WINDOW_BYTES));

  switch (detected) {
    case 'pdf':
      inspectPdf(window, findings);
      break;
    case 'ole':
      inspectOle(window, findings);
      break;
    case 'zip':
      inspectZip(window, findings);
      break;
    default:
      break;
  }

  // An executable is never a document, whatever it was called.
  if (detected === 'executable') {
    findings.push({
      code: 'file.executable',
      summary: 'This is a program, not a document',
      because:
        'Running it is the only thing it does. Nothing inside it will be read as text.',
      hazard: 'stop',
    });
  }

  return {
    declared,
    detected,
    mismatch,
    oversized: false,
    findings,
    hazard: worst(findings),
  };
}

/* ------------------------------------------------------------------ */
/* Reading an inspection out loud                                      */
/* ------------------------------------------------------------------ */

/**
 * One sentence for the card, or null when there is nothing to say.
 *
 * D23: the card states each hazard once. This is the once.
 */
export function describeHazard(inspection: Inspection): string | null {
  if (inspection.oversized) return 'Too large to inspect.';
  if (inspection.findings.length === 0) return null;
  const first = inspection.findings.find((f) => f.hazard === 'stop') ?? inspection.findings[0];
  // Unreachable while the length check above stands — but `noUncheckedIndexedAccess`
  // cannot see that, and a crash inside the sentence that explains a hazard would be
  // the worst possible place for one.
  if (!first) return null;
  const extra = inspection.findings.length - 1;
  return extra > 0 ? `${first.summary} (and ${extra} more).` : `${first.summary}.`;
}
