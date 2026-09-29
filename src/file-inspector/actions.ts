/**
 * Turning a file action into something the gate can judge.
 *
 * **WHY THIS EXISTS.** The classifier has a table: `fs.read` is free, `fs.write`
 * is a confirm, `fs.delete` is a confirm. What that table cannot know is anything
 * about the *file* — what is inside it, whether the target already exists, or
 * whether the destination is somewhere the person would recognise. Those are file
 * facts, and they belong next to the inspection rather than in the classifier's
 * vocabulary.
 *
 * This module is the hinge. It takes a file action plus what the inspector found
 * and produces an `ActionDescriptor` — facts, no policy — which the classifier
 * then judges. It makes no decisions of its own, and the card it builds states
 * each hazard once (D23).
 *
 * **THE FOUR RULES IT ENCODES, each from the threat model:**
 *
 * 1. **Reading is free, and what you read is untrusted** (T12). A read does not
 *    interrupt. But the content it brings in must never be able to authorise
 *    anything, so the read is where the taint is recorded — not on the later
 *    action, which would be too late.
 * 2. **Every write is a confirm, and the card names the destination** (T14).
 *    Always, with no exception for "small" or "temporary". The operator's answer
 *    removed containment, so the gate is the only layer there is.
 * 3. **An existing target is a question, not an inconvenience** (T15). A write
 *    that would replace something is marked irreversible, so the card has to say
 *    what is about to be lost.
 * 4. **Opening is an action** (T13). A file whose structure says it acts is
 *    handed that fact, and the classifier decides what it is worth.
 */

import type { ActionDescriptor, Taint } from '../risk-classifier';

import type { Inspection } from './inspect';

/** The file actions this product takes. There is no fifth. */
export type FileActionKind = 'read' | 'list' | 'write' | 'open';

export interface FileActionInput {
  action: FileActionKind;
  /** The path, as it will be shown to the person. Not resolved here. */
  path: string;
  /** What the inspector found, when the file has been inspected. */
  inspection?: Inspection;
  /**
   * For a write: does the destination already exist?
   *
   * T15 turns entirely on this. An overwrite is not a bigger write; it is a
   * different act, and it is the one that loses something.
   */
  targetExists?: boolean;
  /**
   * Where the *motivation* came from. Anything the agent did because it read a
   * file, a page or a message is `untrusted`.
   */
  taint?: Taint;
  /**
   * Is the destination inside a folder the person nominated?
   *
   * T14: containment was traded away, so a write outside that folder is a write
   * that can reach a share, a sync client, or somewhere else that leaves.
   */
  insideWorkspace?: boolean;
}

export interface FileActionDescription {
  /** The facts, for the classifier. This module decides nothing. */
  descriptor: ActionDescriptor;
  /** Plain-language lines for the card, in the order they matter. */
  notes: string[];
  /**
   * The taint that anything DERIVED from this action inherits.
   *
   * Set by a read, and it is the whole of rule 1. The read itself was asked for
   * by the person and stays `trusted` — escalating the read would interrupt
   * someone for opening a file they chose, which only teaches them to click
   * through. What must never be trusted is what comes OUT: the file's contents
   * are not the person's instruction, so the next action inherits this instead.
   */
  resultTaint?: Taint;
}

const TOOL: Readonly<Record<FileActionKind, string>> = Object.freeze({
  read: 'fs.read',
  list: 'fs.list',
  write: 'fs.write',
  open: 'fs.open',
});

/**
 * Build the descriptor and the card lines for one file action.
 *
 * The order of the notes is deliberate: what is about to happen, then what is at
 * risk. A person reads the first line of a card and skims the rest, so the first
 * line is the act and the second is the cost.
 */
export function describeFileAction(input: FileActionInput): FileActionDescription {
  const notes: string[] = [];
  const taint: Taint = input.taint ?? 'trusted';
  let resultTaint: Taint | undefined;

  const descriptor: ActionDescriptor = {
    tool: TOOL[input.action],
    // The path is an argument, so the destructive-pattern scan sees it and the
    // card can name it. Naming the destination is a T14 requirement, not a
    // formatting preference.
    args: { path: input.path },
    taint,
  };

  switch (input.action) {
    case 'list':
      notes.push(`Listing ${input.path}.`);
      break;

    case 'read': {
      notes.push(`Reading ${input.path}.`);
      // Rule 1. The read stays as the person motivated it — but what comes out
      // is never trusted. The taint travels on `resultTaint`, not on the read:
      // putting it on the read would make the classifier interrupt the person
      // for opening their own file, and a warning that fires on the wrong thing
      // is a warning people learn to dismiss.
      resultTaint = taintFromRead(taint);
      if (input.inspection && input.inspection.hazard !== 'none') {
        notes.push('What it contains is not treated as an instruction from you.');
      }
      break;
    }

    case 'write': {
      // Rule 2. Always a confirm, whatever the size.
      if (input.targetExists) {
        // Rule 3. Replacing something is the case that loses data.
        descriptor.reversible = false;
        notes.push(`${input.path} already exists, and this would replace it.`);
      } else {
        notes.push(`Creating ${input.path}.`);
      }
      if (input.insideWorkspace === false) {
        // T14. A write outside the nominated folder can leave the machine —
        // through a share, a sync client, or anything watching the path.
        descriptor.leavesMachine = true;
        notes.push('The destination is outside the folder you chose.');
      }
      break;
    }

    case 'open': {
      notes.push(`Opening ${input.path}.`);
      // Rule 4. Hand the classifier the only fact it cannot see for itself.
      if (input.inspection?.hazard === 'stop') {
        descriptor.carriesExecutableContent = true;
      }
      for (const f of input.inspection?.findings ?? []) {
        if (f.hazard === 'stop') notes.push(f.because);
      }
      break;
    }
  }

  return { descriptor, notes, ...(resultTaint ? { resultTaint } : {}) };
}

/**
 * Reading makes the *result* untrusted. If the read was already untrusted or
 * mixed, that does not soften; if it was trusted, the file's contents still are
 * not the person's own words.
 */
const taintFromRead = (taint: Taint): Taint => (taint === 'trusted' ? 'untrusted' : taint);
