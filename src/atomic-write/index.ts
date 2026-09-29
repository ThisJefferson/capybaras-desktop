/**
 * Writing a file so a crash cannot leave half of one.
 *
 * (T15 — overwrite and destruction. Stage 3.3 of the safe-file model.)
 *
 * **THE THREAT.** A write that fails partway through leaves a truncated file. The
 * person then has neither the old content nor the new: the document is not
 * corrupted in a way they can see, it is simply *shorter*, and the lost half is
 * gone. This is the failure mode that eats work, and it is the one nobody
 * notices until they open the file.
 *
 * **WHAT THIS DOES.** Temp-and-rename. The whole content is written to a sibling
 * temporary file, flushed to disk, and then `rename()`d over the target. On one
 * volume `rename` is atomic: the target is either the old file or the new one,
 * **never a mixture and never truncated.**
 *
 * **TWO FUNCTIONS, AND THE DIFFERENCE IS THE ENTIRE POINT.**
 *
 * - **`writeAtomic`** replaces the target. This is for **our own** files — grants,
 *   the sidecar's state — where the file exists to be updated and replacing it is
 *   the whole idea.
 * - **`writeNewOnly`** **refuses if the target exists.** This is for anything a
 *   person did not ask us to replace. An existing file is a *question*, not an
 *   inconvenience (T15).
 *
 * Getting these two confused is how a product silently eats someone's work, so
 * they are separate functions with separate tests rather than one function with a
 * boolean.
 *
 * **AND THIS IS THE PRIMITIVE, NOT THE POLICY.** It does not decide whether a
 * write should happen — that is the gate's job (`src/file-inspector/actions.ts`
 * builds the question, `src/policy/gate.ts` answers it). This module only makes
 * the write itself safe. It never asks, never prompts, and never judges a path.
 *
 * **WHAT IT DELIBERATELY DOES NOT DO.** It does not create parent directories,
 * resolve symlinks, check permissions, or enforce any path policy. Every one of
 * those is a decision, and decisions live above this layer.
 */

import { closeSync, fsyncSync, openSync, renameSync, unlinkSync, writeSync } from 'node:fs';

/** The result of a write. A failure is a value, never an exception. */
export type WriteOutcome =
  | { readonly ok: true; readonly path: string }
  /** `writeNewOnly` found something already there. Nothing was touched. */
  | { readonly ok: false; readonly reason: 'exists'; readonly path: string }
  /** The write could not be made. Nothing was left behind. */
  | { readonly ok: false; readonly reason: 'failed'; readonly path: string; readonly detail: string };

/**
 * The outcome of `writeAtomic`, which **cannot** report `'exists'` — it replaces
 * by design, so there is nothing to find in the way.
 *
 * Kept narrower than `WriteOutcome` on purpose: a caller should not be able to
 * write a branch for a case that cannot happen, and should not be able to
 * *forget* the case that can. The first version of this shared one union and a
 * caller could not safely read `detail`; the type was wrong, not the caller.
 */
export type AtomicOutcome = Exclude<WriteOutcome, { readonly reason: 'exists' }>;

/** A temporary sibling path that cannot realistically collide with another. */
const temporaryFor = (path: string): string =>
  `${path}.${process.pid.toString(36)}${Date.now().toString(36)}.tmp`;

/** Write bytes and flush them, so the rename that follows cannot land before them. */
const writeAndFlush = (path: string, content: string): void => {
  const fd = openSync(path, 'w', 0o600);
  try {
    writeSync(fd, content, null, 'utf8');
    // Without this the rename can be durable while the CONTENT is not, which
    // reintroduces exactly the failure this module exists to prevent.
    fsyncSync(fd);
  } finally {
    closeSync(fd);
  }
};

/**
 * Replace `path` with `content`, atomically.
 *
 * The target is either untouched or fully replaced. A crash between the two
 * steps leaves the temporary file behind and the target exactly as it was —
 * which is why the cleanup below matters, and why it never deletes the target.
 */
export function writeAtomic(path: string, content: string): AtomicOutcome {
  const temporary = temporaryFor(path);
  try {
    writeAndFlush(temporary, content);
    // Atomic on the same volume. If this throws, the target is still the old file.
    renameSync(temporary, path);
    return { ok: true, path };
  } catch (error) {
    // Best effort: a leftover temp file is untidy, but the original is intact.
    // The failure to delete is deliberately swallowed — reporting it would
    // replace a real error with a less important one.
    try {
      unlinkSync(temporary);
    } catch {
      /* nothing left to do */
    }
    return {
      ok: false,
      reason: 'failed',
      path,
      detail: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Create `path` with `content`, **but only if nothing is there.**
 *
 * Uses an exclusive open rather than a check-then-write, because the gap between
 * checking and writing is exactly where a file created by someone else gets
 * clobbered. `'wx'` makes the existence check and the creation a single
 * operation the operating system performs for us.
 *
 * When something is already there, **nothing is written and nothing is
 * removed** — the caller gets a value it can turn into a question on a card.
 */
export function writeNewOnly(path: string, content: string): WriteOutcome {
  let fd: number;
  try {
    fd = openSync(path, 'wx', 0o600);
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code === 'EEXIST') return { ok: false, reason: 'exists', path };
    return {
      ok: false,
      reason: 'failed',
      path,
      detail: error instanceof Error ? error.message : String(error),
    };
  }
  try {
    writeSync(fd, content, null, 'utf8');
    fsyncSync(fd);
    return { ok: true, path };
  } catch (error) {
    closeSync(fd);
    // We created it, so we own the mess. Remove the partial file rather than
    // leaving a damaged one where the caller believes nothing was written.
    try {
      unlinkSync(path);
    } catch {
      /* nothing left to do */
    }
    return {
      ok: false,
      reason: 'failed',
      path,
      detail: error instanceof Error ? error.message : String(error),
    };
  } finally {
    try {
      closeSync(fd);
    } catch {
      /* already closed on the error path */
    }
  }
}
