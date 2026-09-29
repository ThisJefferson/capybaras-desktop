/**
 * The card's voice.
 *
 * (Gap 7.7, from `docs/plans/fix-the-gaps-2026-09-29.md`.)
 *
 * **WHY THIS EXISTS.** The approval card is the product. It is the one screen
 * that has to work for someone who does not know what a context length is, and
 * the one screen where a technical word costs the most. The card was shipping
 * the tag **"Hard gate"** — our name for a mechanism, shown to a person at the
 * exact moment they are deciding whether to let something happen.
 *
 * **THE RULE.** The card describes **what will happen**, never **how we classify
 * it**. "Hard gate" is our word. "This one needs care" is theirs. If a sentence
 * on the card would only make sense to someone who has read our threat model,
 * it is the wrong sentence.
 *
 * **WHY IT IS A MODULE AND NOT THREE STRINGS.** Strings drift. Someone adds a
 * tier, copies the old tag, and the jargon is back. These are tested, and the
 * glossary means that when a technical word *does* have to appear — in a reason
 * the classifier wrote, say — there is one place to look up how to say it
 * plainly, rather than a person leaving the decision to go and search.
 */

import type { RiskTier } from '../risk-classifier';

/**
 * What the card calls each tier, if it says anything at all.
 *
 * Two of these are empty on purpose. `silent` never reaches a card, and
 * `notify` is a message rather than a question — a label there would be noise on
 * a screen that already has one sentence to read.
 */
export const TIER_LABEL: Readonly<Record<RiskTier, string>> = Object.freeze({
  silent: '',
  notify: '',
  confirm: '',
  hard_gate: 'Needs care',
});

/**
 * The one line that follows the label, in the card.
 *
 * It describes the *difference the person will notice* — they will be asked to
 * type, and this approval will not be remembered — rather than the rule behind
 * it. Both halves are things they will experience directly, which is what makes
 * them worth saying.
 */
export const HARD_GATE_NOTE =
  'You will type a phrase rather than click, and Capybaras will not remember this one.';

/**
 * The words we use that a person would not.
 *
 * This is a safety net, not a vocabulary list. Every entry here is something the
 * card should be *avoiding*; it exists so that when one leaks — from a reason,
 * a source document, or a future mistake — there is a plain reading of it in the
 * one place a person is already looking.
 *
 * Ordered longest-first for lookup, because "blast radius" must not be matched
 * by a rule for "radius".
 */
export const GLOSSARY: ReadonlyArray<{ readonly term: string; readonly plain: string }> =
  Object.freeze([
    {
      term: 'blast radius',
      plain: 'How many things one action would touch at once.',
    },
    {
      term: 'protected target',
      plain: 'Something you have marked as off-limits.',
    },
    {
      term: 'hard gate',
      plain: 'A decision Capybaras will never make on its own or remember for you.',
    },
    {
      term: 'irreversible',
      plain: 'Once it happens, it cannot be undone.',
    },
    {
      term: 'reversible',
      plain: 'It can be undone afterwards.',
    },
    {
      term: 'taint',
      plain: 'The request came from something Capybaras read, not from you.',
    },
    {
      term: 'exfiltration',
      plain: 'Content being sent somewhere you did not choose.',
    },
    {
      term: 'exfiltrate',
      plain: 'To send content somewhere you did not choose.',
    },
  ]);

/**
 * A plain reading of a technical term, or `null` when we do not have one.
 *
 * Matching is case-insensitive and whole-word, so "Hard gate" and "hard gates"
 * both resolve while an unrelated word containing the same letters does not.
 * Returning `null` rather than a guess is deliberate: a wrong explanation on the
 * card is worse than no explanation, because it is trusted.
 */
export function explain(term: string): string | null {
  const needle = term.trim().toLowerCase();
  if (!needle) return null;
  for (const entry of GLOSSARY) {
    // Word-boundary match, built from the escaped term so no entry can turn
    // itself into a pattern by accident.
    const escaped = entry.term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    if (new RegExp(`\\b${escaped}s?\\b`, 'i').test(needle)) return entry.plain;
  }
  return null;
}

/**
 * The label a tier gets on the card, and the note that follows it.
 *
 * Returns `null` when the tier should not be labelled at all, which is most of
 * them — the card is a decision, not a status readout.
 */
export function tierCopy(tier: RiskTier): { label: string; note: string } | null {
  const label = TIER_LABEL[tier];
  if (!label) return null;
  return { label, note: tier === 'hard_gate' ? HARD_GATE_NOTE : '' };
}
