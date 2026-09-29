# Approval Card — Design Specification

The approval card is the product's hero component. It is what a non-technical person sees when the agent wants to do something risky. Every design decision below exists to answer one question: **how do you make a person stop, read, and make a deliberate choice without alarming them?**

---

## 1. Structure

The card is a centred white panel on the sand (`#FBF7F0`) background, 780 px wide by 540 px tall. It contains, from top to bottom:

1. **Coral left accent bar** — a 6 px wide strip of coral (`#F2705A`) running the full height of the card. It tells the eye "this card is different" before the user reads a single word.
2. **Zeca the capybara** — the needs-you capybara (sign raised, standing) positioned in the top-left interior. Zeca is the capybara who checks the work; seeing Zeca with the sign signals "something was double-checked and needs your input."
3. **"Tuca needs you" header** — coral, small, letter-spaced. Establishes who is requesting.
4. **Headline** — one plain sentence answering "what will happen?" in the user's language. No tool names, no technical identifiers. Examples: *"Send a draft to maria@example.com?"* or *"Delete 3 files from Documents?"*
5. **Target line** — the target of the action in plain terms (optional, appears when relevant).
6. **Reversibility line** — if the action cannot be undone, this appears as a bold warning before the reasons. If it can be undone, this line is absent. It is never hidden when true.
7. **Reasons section** — every reason the classifier found, listed as short bullet lines. Always all of them, never truncated.
8. **Divider line** — a thin `#D8D2C6` line separating the explanation from the action.
9. **Action buttons** — two buttons side by side.
10. **"Always allow" affordance** — a collapsed accordion at the bottom of the card.

### 1.1 Single reason vs multiple reasons

When there is one reason, the reasons section occupies a single line and the card is proportionally shorter (the layout compresses from the bottom). When there are three reasons, each occupies its own line and the card maintains its fixed height with the same internal spacing. The design never truncates reasons, never shows a scrollbar within the card, and never forces a user to expand a hidden section to see a reason. For more than three reasons, the body of the card becomes scrollable **only** within the reason-and-description area, keeping the buttons always visible. The practical maximum from the classifier is six reasons; a card with six reasons scrolled reveals all of them without losing the buttons.

### 1.2 Tiers: confirm vs hard gate

The card has two visual modes corresponding to the two tiers:

**Confirm tier**: the card described above. Two buttons, one clearly default.

**Hard gate tier**: everything above, plus:
- The "Go ahead" button is replaced by a **text input field** and a smaller **"Type to confirm" button** that is disabled until the user types the exact confirmation phrase.
- The confirmation phrase is printed on the card so the user knows what to type.
- The "Always allow" affordance is **absent** — hard gates can never be remembered.
- The `Hold on` button remains identical.

Both tiers are handled by the same card component; the hard gate only adds UI elements rather than replacing them.

---

## 2. Button primacy — why "Hold on" gets visual primacy

**The safe choice is the easy one. "Hold on" is the default, filled, visually heaviest button. "Go ahead" is an outlined secondary button.**

This is intentional and deliberately inverts the common Cancel/OK pattern used by most operating system dialogs. The reasoning:

### 2.1 The product's promise

Capybaras *"ask before they act."* The entire value proposition is that nothing happens automatically. A card that makes "proceed" the path of least resistance betrays that promise at the moment it matters most. If a user glances at the card and hits Enter out of habit, and that Enter fires "Go ahead," the product has just done the thing it exists to prevent.

### 2.2 Habit and muscle memory

Most software installs a reflex: Enter / Space bar = confirm, accept, continue. People hit Enter on dialog boxes without reading them constantly. The approval card must **interrupt that reflex** — not exploit it. Making "Hold on" the primary filled button and wiring Enter to "Hold on" means the reflex action is the safe one. A user who acts without reading **cannot accidentally approve something destructive.** They can only accidentally stop the action, which costs nothing and can be corrected instantly.

### 2.3 The cost of a false positive vs a false negative

If the user hits "Hold on" when they meant to approve, nothing bad happens. The action is paused, they can review and proceed. If they hit "Go ahead" when they meant to stop, the damage may be irreversible. The button layout must bias toward the cheaper error.

### 2.4 Visual weight

- "Hold on": filled atlantic (`#0F5C63`), white text, bold weight, 200 x 50 px.
- "Go ahead": outlined (`stroke: #D8D2C6`), grey text (`#6B6B6B`), regular weight, same size.
- A label below "Hold on" reads: *"default - press Enter"* in 10 px grey, reinforcing the keyboard behaviour.

### 2.5 Keyboard handling

Enter, Space bar, and the Return key all trigger "Hold on." "Go ahead" has no keyboard shortcut. The user must explicitly click it (or tab to it and press Space) — another deliberate friction point.

### 2.6 Hard gate variant

For hard gates, the "Type to confirm" button is the *only* way to proceed. There is no direct "proceed" click. The user must read the phrase, type it, and submit. "Hold on" remains the default action.

---

## 3. The "always allow something like this" affordance

A remembered grant can satisfy a `confirm` tier request and **nothing else.** A hard gate is never satisfied by memory, no matter how many times a user approved it. The grant store enforces this at the code level (`grants.ts`: *"A grant can never be created for a hard gate."*).

### 3.1 UI placement

The affordance lives **below the action buttons**, not beside them. It is a collapsed accordion row with a small disclosure arrow and the text *"Remember this action for 30 days"*. It is deliberately small, low-contrast, and out of the natural reading flow:

```
[                          ]
  > Remember this action for 30 days
```

Clicking it expands to show a brief explanation and a toggle:

```
  v Remember this action for 30 days

  If you approve, this exact action (delete file
  "notes.txt" from Documents) will skip asking next
  time. It expires in 30 days. You can revoke it
  anytime in Settings.

  [ Remember on approve ]
```

### 3.2 Why it is deliberately harder than approving once

Three design decisions create this friction:

1. **Placement below the fold.** The user must scroll or look past the buttons to find it. It is never in the same visual plane as the primary decision.

2. **No direct effect.** Checking the box does not approve the current action. It only signals that *if* the user also clicks "Go ahead," the system should remember. The current action must still be approved separately. This prevents one-click "approve forever" reflexes.

3. **Explicit expiration.** The affordance always states the 30-day expiry in the label itself, reminding the user this is a temporary grant, not a permanent exemption.

### 3.3 Why grants cannot satisfy anything above confirm

The classifier determines `allowRemember` based on:
- Tier must be exactly `confirm` (not `hard_gate`)
- Motivation must be `trusted` (not from content the agent read)
- Action must be `reversible` (not something that cannot be undone)
- Target must not be `protected`

A grant is scoped to one tool, one exact target, with no wildcards. You cannot say "always allow deletes in Documents." You can say "always allow delete `notes.txt` from Documents." This granularity is enforced in `grants.ts` and reflected in the UI by showing the exact scope in the expandable row.

### 3.4 Settings visibility

All active grants appear in a dedicated Settings panel titled "Remembered choices." Each grant shows the action, the target, and the remaining days. Each can be revoked individually. A "Revoke all" button clears everything at once. This is the only place where grants can be reviewed without an approval card open.

---

## 4. Reasons: showing everything, never hiding

The classifier may produce up to six reasons for a single action. Every one of them appears in the card. Reasons are short, plain-language sentences. Examples from the classifier:

- *This cannot be undone.*
- *This would reach outside your computer.*
- *The instruction for this came from content the agent read, not from you.*
- *An untrusted source asked for something that can destroy or send.*
- *This would affect 50 things at once.*
- *This matches a destructive command pattern (recursive force delete).*

### 4.1 Layout

Each reason is preceded by a small coral dot (3 px circle, `#F2705A` — matching the needs-you colour) and rendered in 15 px `#4A4A4A` text. The dot colour ties the reasons visually to the "needs you" state, reinforcing that this is the agent asking for human help.

### 4.2 Deduplication

The classifier may produce "This cannot be undone" and also a second reason that implies irreversibility (e.g., "This cannot be undone, and it affects many things"). When the classifier produces both `reversible === false` and the combined catastrophic reason, the card shows both. They are separate findings and hiding one would defeat the purpose of surfacing every reason. If two reasons are near-identical, the risk-classifier is the correct place to deduplicate, not the UI.

### 4.3 No tool identifiers

The classifier produces plain-language reasons without tool names or internal identifiers. The card never adds them. A `fs.delete` action reads as *"Delete 3 files from Documents"* in the headline, and the reasons are the classifier's output verbatim.

---

## 5. Reversibility: unmistakable when irreversible

When `action.reversible === false` (or the classifier added a reason about irreversibility), the card shows a bold warning line immediately below the headline and target, before the reasons section:

```
This cannot be undone.
```

This line uses:
- Font weight: bold (700)
- Font size: 15 px (slightly smaller than the headline, larger than reasons)
- Colour: ink (`#161616`) — the same as the headline, not coral. Coral is reserved for the "needs you" indicator, not applied to warnings. Using ink makes the warning feel factual and serious rather than alarming.
- Icon: a small filled rhombus (◆) in atlantic (`#0F5C63`) preceding the text, subtly different from the reason bullets.

The warning is **not repeated** in the reasons section (the classifier would have added it as one of the reasons). But because reversibility is so important, it gets a dedicated top-level line instead of being buried in a list. The reasons section may still include the classifier's irreversibility reason — the line and the reason coexist rather than replacing each other.

When the action **can** be undone, no reversibility line appears. The absence communicates "this is safe" without needing to say it.

### 5.1 For hard gates

Irreversibility alone does not make a confirm into a hard gate. But when a hard gate is combined with irreversibility (which is common — deleting files with untrusted motivation), the "This cannot be undone" line appears with extra prominence: a coral (`#F2705A`) warning strip behind the text, 2 px tall, spanning the width of the content area, with the text itself in coral. This is the only circumstance where coral appears on text that is not the "needs you" header — because "cannot be undone" on a hard gate is the most important thing the card communicates.

---

## 6. Coral usage — reserved for "needs you"

Per the brand palette rule, coral (`#F2705A`) appears on the approval card in exactly these places:

| Element | Why coral |
|---|---|
| Left accent bar (6 px wide, full card height) | Makes the card unmistakable as a "needs you" element |
| "TUCA NEEDS YOU" header | Identifies the state |
| Zeca's sign border and stick | Matches the brand capybara needs-you SVG |
| Reason bullet dots (3 px circles) | Ties the reasons section visually to the needs-you state |
| Hard gate irreversibility strip (text and background line) | Only circumstance where coral appears on a warning, and only because the hard gate + irreversibility combination is the highest-risk scenario the card can show |

Nothing else on the card uses coral. Not the reversibility line, not the "Hold on" button, not the "always allow" affordance. Coral means "a capybara is waiting for you" — every other element defers to the brand palette (atlantic, ink, sand, leaf, sky, capy brown).

**Colour values used**, verified against brand assets:
- Coral: `#F2705A`
- Atlantic (primary button): `#0F5C63`
- Sand (background): `#FBF7F0`
- Ink (body text): `#161616`
- Capy (capybara body): `#8B6239`
- Capy dark (capybara legs/ear): `#7A5530`
- Capy nose: `#5C4023`
- Sky (card surface): `#CFE7E6`
- Leaf (success accent, not used on card): `#4E8C5A`
- Divider: `#D8D2C6`
- Secondary text: `#4A4A4A`
- Muted text: `#8B8B8B`

---

## 6.1 The mascot on the card — signal, never opine

**The card has a capybara on it, and that is deliberate.** Zeca holds a raised sign, and **the raised sign *is* the "needs you" status** — the same posture the herd shows everywhere else. A mascot that reports state is not decoration; it is the status display, and removing it would remove information.

> **CORRECTION, 2026-09-29.** `docs/plans/fix-the-gaps-2026-09-29.md` (gap 7.5) proposed *"the permission card is a mascot-free zone."* **That was wrong, and executing it is what showed it was wrong.** The card already uses the capybara *as the status signal*, which is exactly the TunnelBear pattern — the bear digs, and the digging is the connection state. The problem was never that the mascot appears; it is what the mascot might *do* there. The rule below replaces the proposal.

**The rule, stated so it cannot drift:**

> **The herd may SIGNAL. It may never OPINE.**

**Signal** — a posture that reports a fact: *someone needs you*; *working*; *finished*. The same posture, drawn the same way, **every time**, whatever is being asked and whatever the person decides.

**Opine** — anything that reacts to the *answer*. The capybara must not look pleased when someone allows something, disappointed when they decline, relieved, worried, or eager. It must not lean toward a button, look at the headline, or change with the tier.

**Why this matters more here than anywhere else in the product.** The card is the one screen where a person decides whether to let something happen. **A mascot that appears to want a particular answer is a second opinion on that decision** — from us, the party asking permission, expressed as a feeling rather than a reason. **We are the ones asking. We do not get to look hopeful.**

**How it is enforced.** The needs-you posture is drawn from a fixed description and is **identical in every context** (see the drawing note in `apps/desktop/web/app.js`). The card passes the tier, the headline and the outcome nowhere near the drawing, and that must stay true: **a posture drawn from the decision would be the bug.**

**The corollary for everything else:** charm lives everywhere the person is *not* deciding. The card says what will happen, and then gets out of the way.

---

## 7. Layout measurements

### 7.1 Card dimensions

| Property | Value |
|---|---|
| Width | 780 px |
| Height | 540 px (fixed; scrolls internally if more than 3 reasons) |
| Corner radius | 18 px |
| Shadow | `0 6 16 #0F5C63` at 22% opacity |
| Left accent bar | 6 px wide, corner radius 3 px, full height |
| Horizontal padding (content) | 28 px from card edge |
| Top padding | 28 px |

### 7.2 Content spacing

| Element | Y position (from card top) | Size |
|---|---|---|
| Zeca capybara | 24 px, 32 px from left edge | scale 0.52 |
| "TUCA NEEDS YOU" header | 28 px | 11 px, letter-spacing 1.3 |
| Headline | 68 px | 20 px, bold |
| Target line | 94 px | 16 px |
| Reversibility line | 120 px | 15 px, bold |
| Reasons header ("Why") | 148 px | 11 px, letter-spacing 1.0 |
| Reason 1 | 172 px | 15 px |
| Reason 2 | 198 px | 15 px |
| Reason 3 | 224 px | 15 px |
| Divider | 268 px | 1 px |
| Button row | 290 px | 50 px tall |
| Default label | 352 px | 10 px |
| Always allow affordance | 380 px | expanded ~80 px |

### 7.3 Responsive behaviour

On narrow screens (below 780 px), the card becomes full-width with 16 px horizontal margins. Zeca moves above the headline rather than beside it. The two buttons stack vertically ("Hold on" above "Go ahead") to prevent accidental taps. The always allow affordance stays at the bottom.

---

## 8. Visual polish

- The card shadow uses atlantic with low opacity (not generic black) for a teal tint.
- The coral bar has rounded outer corners matching the card's corner radius.
- Zeca's sign text reads *"oi!"* in coral on a sand background, matching the brand needs-you SVG exactly.
- The capybara is positioned so Zeca faces inward toward the content, creating a natural reading direction from mascot to text.
- All text is left-aligned except the tagline below the wordmark. Right-to-left layout support is a separate project phase.

---

## 9. Open questions

- Should the "always allow" affordance have a visual confirmation animation when checked? (Proposal: a brief coral pulse on the left accent bar, reusing the needs-you colour.)
- For hard gates, the typed confirmation field shows the target phrase but does not mask it. Should it mask the first character to prevent shoulder-surfing? (Trade-off: masking adds friction in a flow that is already designed for friction.)
- What happens when a grant expires mid-operation? (Proposal: the gate re-checks before every action, not once at the start. An expired grant produces a fresh approval card.)
---

## Reviewer's note (2026-09-27)

Added by the parent session because the drafting run **timed out before it could
verify its own output**, so the artifact was reviewed rather than trusted.

**Verdict: it holds up.**

- Coral (`#F2705A`, 8 uses) is confined to the needs-you surface and is **not**
  used for the approve control. The primary button is Atlantic (`#0F5C63`), which
  is the right call: the safe choice carries the visual weight, and Enter maps to
  "Hold on" rather than "Go ahead". The reflex action is the harmless one.
- The hard-gate variant removes the always-allow affordance entirely, which
  correctly encodes the invariant that a hard gate is never satisfied by memory.
- The remember affordance sits below the fold and has no direct effect, so
  "approve forever" cannot be reached by reflex.

**One deviation to settle before M4.** The SVG uses five neutrals that BRAND.md
does not declare — `#B0B0B0`, `#4A4A4A`, `#6B6B6B`, `#8B8B8B`, `#FFFFFF` — plus
two browns derived from `#8B6239` (`#7A5530`, `#5C4023`). Deducible and defensible
for UI text and borders, but undeclared. Either add a neutral ramp to BRAND.md or
tighten the card. Not blocking.
