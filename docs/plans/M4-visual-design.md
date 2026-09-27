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

### Step 6 — Mascots: spike the options, then decide
This is the one that may cost money, so it gets decided with evidence rather than enthusiasm. Three candidate approaches:

| Approach | Quality ceiling | Cost | Risk |
|---|---|---|---|
| **Disciplined vector work** (Figma, proper iteration) | Clean and intentional | Free | Not illustrator-grade; will always read as "designed by an engineer" |
| **Generative raster** (11 models already available) | Can be genuinely good | Free | **Consistency across six characters × four states is the hard problem.** Output licensing must be checked before shipping |
| **Commissioned illustration** | The actual ceiling | Real money | Needs a budget and a brief |

**Recommendation:** spike the first two, compare honestly against the third as a benchmark, and put the commission on the roadmap as the first thing funded when there is revenue. Good mascot art is the one part of this project you genuinely pay for — and saying so is more useful than pretending otherwise.

**Exit:** a written comparison with actual outputs side by side, and a decision with its reasoning recorded.

### Step 7 — The review loop and the quality bar
Screenshot the **running app** (not the mockup), put it next to reference products, and iterate. Define the bar explicitly:

> **Would this look at home in the Microsoft Store next to a paid application — or does it look like a weekend project?**

Repeat until the honest answer is the first one. This loop is where most of the actual quality comes from, and it is free.

**Exit:** a documented before/after, and the bar written into `DECISIONS.md`.

---

## What this needs from Jeff

Only one real decision, and it can wait:

1. **The mascot budget, when there is revenue.** Until then the interface will look *clean and intentional*, and the mascots will be *competent* — not beautiful. **Everything else in this plan is free and can start immediately.**
2. **Are the mascots load-bearing or decorative?** If they are decoration, the floor is much higher and we can be patient. If the brand genuinely depends on six charming capybaras, that is a commission and it should be planned for.

---

## Ordering, and what it does for M4

Steps 1–4 are prerequisites for a good approval interface and are free. Steps 5–7 make it feel finished.

**The honest summary:** the reason the current visuals look basic is not a shortage of effort — it is that hand-authored SVG is not illustration, and that there is no design system underneath yet. **The system is the fix, it is free, and it does most of the work.** The capybaras are the part worth paying for later.
