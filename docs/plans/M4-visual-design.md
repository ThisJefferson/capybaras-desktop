# M4 — Visual design: making it beautiful, not merely functional

**The question this answers:** we have a mockup and some hand-made SVGs. How does this become something that looks like a real product rather than a prototype?

Compiled 2026-09-27, at Jeff's request.

---

## The honest diagnosis

The current assets are **placeholder-grade**, and it is worth saying why rather than pretending otherwise.

**I write SVG paths by hand.** That produces shapes with inconsistent curve quality, no optical correction, no character, and no craft in the line work. Adding more paths will not fix it — the limit is not effort, it is that hand-authored geometry is not illustration. A professional mascot comes from an illustrator iterating on a drawing over several passes. That is a different activity from what generated `capy-sleep.svg`.

So the plan has to be honest about two separate problems, because they have different solutions and very different costs:

| Problem | Difficulty | Cost |
|---|---|---|
| **The interface is not a design system** | Very tractable | Free |
| **The mascot art is amateur** | Genuinely hard | Money, probably |

Conflating them is how projects end up spending on logo polish while the app still looks like a prototype.

---

## Where beauty actually comes from

This is the most useful thing in this document, and it is counter-intuitive:

**Beautiful software is roughly 80% typography, spacing, colour discipline, and motion — and about 20% illustration.**

People believe the mascots make TunnelBear charming. They don't. TunnelBear is charming because of confident type, generous spacing, a restrained palette, and animation that has *timing*. The bear could be a plain circle and it would still feel like a product.

The corollary, and it is the design principle for this whole phase:

> **Design the interface so it does not depend on the art being excellent.** A good layout with modest illustrations reads as intentional. A bad layout never reads as good, no matter what you put in it.

That means the mascots are a **garnish we can afford to be patient about** — not a blocker for looking professional.

---

## What we already have to work with

Verified on this machine, not assumed:

- **11 image-generation models are available** through the OpenRouter account already configured (Gemini 3 Pro Image, GPT-5 Image, and others). So a generative path for illustration exists today, at no new cost.
- **The palette is settled** (`BRAND.md`): sand `#FBF7F0`, atlantic `#0F5C63`, coral `#F2705A` (reserved for *needs you*), capy `#8B6239`, leaf `#4E8C5A`, ink `#161616`, sky `#CFE7E6`.
- **Tauri renders a real webview**, so the interface is standard HTML/CSS — no exotic UI toolkit, and every modern CSS technique is available.
- **A real approval card design exists** (`docs/design/approval-card.md`), already reviewed, with the button-primacy decision settled.

---

## Steps

### Step 1 — Tokens, and fix the undeclared palette
Expand `BRAND.md` into a real token set: a **type ramp**, a **spacing scale** (4/8pt grid), **radii**, **elevation**, **semantic colours**, and the **neutral ramp** the approval-card review flagged as missing. Five greys and two browns were being used without being declared, which turns a palette into a suggestion.

**Exit:** `design/tokens.json`, referenced by both the docs and the app. No hard-coded hex anywhere in a component.

### Step 2 — Typography
The single largest lever, and the cheapest. Choose a text face and a display face — both **OFL licensed so they can be bundled legally** — and set a real type scale with intentional line-height and measure. Stop relying on the default system stack for everything.

**Exit:** documented type ramp, applied, with a side-by-side against the current default rendering.

### Step 3 — Adopt a professional icon set
**Stop hand-drawing icons.** Use an established, open-licensed set — Lucide (MIT) or Phosphor (MIT) — for a consistent 24px grid, uniform stroke weight, and optical consistency. This is exactly the "use existing solutions" rule: thousands of hours of craft already exist, are free, and are better than anything generated here.

**Exit:** the icon dependency adopted, documented in `THIRD_PARTY_NOTICES.md`, and the hand-made icons retired.

### Step 4 — A component kit and a review surface
Build components on the tokens, and add a **hidden review route** that renders every state in one place: all four herd states, both card tiers, empty/loading/error, the grant dialog, the receipt. This is what makes design review systematic instead of a screenshot lottery.

**Exit:** one page showing every component and state, viewable without hunting for it.

### Step 5 — Motion
Professional feel lives disproportionately in motion. Define **durations** (short, eased, never showy), **easings**, and one **signature moment** — the transition into *needs you*, which is the emotional centre of the product. Get that one exactly right; keep everything else quiet.

**Exit:** a motion spec, plus the *needs you* moment implemented and reviewed.

### Step 6 — Mascots: spiked, and the answer is in (spike 003)

**Status: done**, and the premise changed twice. Jeff confirmed the capybaras are **integral to the product**, not decoration — which raises the bar rather than lowering it. Then Jeff proposed **one character design, duplicated six times** (recorded as `DECISIONS.md` D18), which is the standard mascot pattern *and* removes the blocker this spike found.

Generation was **tested rather than debated**: three images via `gemini-3-pro-image`, **$0.14 and ~20 s each**, evaluated with a task-specific vision prompt. Full detail: [`spikes/003-mascot-generation/README.md`](../../spikes/003-mascot-generation/README.md).

**Verdict: PARTIAL.** Generation is excellent for *exploring* a direction and not good enough for *shipping* a set:

- **The same prompt produced two different capybaras** — different muzzle, head shape, belly patch, line weight, palette. That is continuity drift, and a herd needs **24 mutually consistent poses**.
- **Zeca's sign sat inside his torso silhouette** — no silhouette separation, so the prop, the hand and the belly patch collapse into one blob at 64 px. That is the most basic requirement for an icon, failed.
- Malformed hands, floating debris (a deformed folder icon, a stray cursor, stray dots), and gibberish badge detail that turns to one-pixel noise when scaled down.

**The revised strategy — and it changes what we buy:**

1. **Generate to explore.** At $0.14 an image, a hundred-image exploration pass costs about **$14**. That replaces mood boards and sharpens the brief. It should be done *before* any money is spent, and it is the capability generation is genuinely good at.
2. **Commission ONE character model sheet — four poses and the sign.** Per D18 the design is a single capybara duplicated six times, so the real asset is one canonical drawing: front, side, expression range, and the sign. That is a **small, standard, quotable job** — plausibly an illustrator-day — rather than a character-set commission. The reduction from 24 poses to 4 is what makes it affordable.
3. **The sheet also cures the consistency problem.** With a locked canonical reference, derivative poses become viable *image-to-image* — matching a reference instead of inventing a character from text every time. **The sheet is what makes the cheap path work.**

**Exit:** a direction chosen from generated exploration, and the **one-character** model-sheet commission briefed and quotable.

### Step 7 — The review loop and the quality bar
Screenshot the **running app** (not the mockup), put it next to reference products, and iterate. Define the bar explicitly:

> **Would this look at home in the Microsoft Store next to a paid application — or does it look like a weekend project?**

Repeat until the honest answer is the first one. This loop is where most of the actual quality comes from, and it is free.

**Exit:** a documented before/after, and the bar written into `DECISIONS.md`.

---

## What this needs from Jeff

1. **The mascot commission — now confirmed as planned spend, and much smaller than it was.** You've said the capybaras are integral, which settles that it should be done properly; D18 then cut it from a character set to **one character × four poses + one sign**. What it needs next is an **art brief** (which the generated exploration produces) and a **quote**.
2. **Everything else in this plan is free and unblocked** — steps 1–5 and 7 can start immediately. The interface will look clean and intentional long before the mascots are beautiful, which is the correct order: the system carries the quality, the illustration is the garnish.

---

## Ordering, and what it does for M4

Steps 1–4 are prerequisites for a good approval interface and are free. Steps 5–7 make it feel finished.

**The honest summary:** the reason the current visuals look basic is not a shortage of effort — it is that hand-authored SVG is not illustration, and that there is no design system underneath yet. **The system is the fix, it is free, and it does most of the work.** The capybaras are the part worth paying for later.
