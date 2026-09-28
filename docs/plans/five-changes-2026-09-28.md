# Plan — the five changes, 2026-09-28

Written after Jeff's run at 12:18. **Every cause below was verified in the code before this plan was written** — none is inferred from the symptom.

## What is actually wrong

| What he saw | Cause, with the line |
|---|---|
| *"I could only read maybe two paragraphs"* of a research paper | `apps/desktop/src-tauri/src/chat.rs:30` — `pub const MAX_REPLY_TOKENS: u32 = 256;`. The request asks the API for at most 256 tokens. Two paragraphs. The display cap `REPLY_MAX = 8000` chars is fine; the truncation is **upstream, at the request**. |
| *"no pricing info next to the models other than Free"* | `src/onboarding/models.ts` — `CatalogEntry` carries `id`, `name`, `description`, `provider`, `link`, `contextLength`. **There is no price field at all.** The catalogue supplies pricing; the parser drops it. |
| *"there's no real animation going on"* | `apps/desktop/web/app.js:441` — the file says it itself: *"The character art is a PLACEHOLDER. The calçadão wave mark stands in until…"* The herd draws one abstract SVG wave per agent. **There are no capybara characters yet.** |
| *"now all of the free models don't work"* | Two real causes. (1) `chat.rs:181` has an empty-reply path — a 200 with no content is a failure the **first sweep's criterion never checked**; it accepted any HTTP 200. (2) The free set contains entries that are not chat models at all — `google/lyria-3-clip-preview` and `google/lyria-3-pro-preview` are music-generation models, priced at zero. |

---

## A — real animated capybaras

**Today:** one abstract wave mark per agent. **Wanted:** capybara characters that move and do things.

**Approach: hand-authored SVG characters, animated with transform and opacity only.** No new dependency, no raster assets, and it satisfies the motion rules the UI pass established.

- Each herd member becomes a small character assembled from a few parts — body, head, ears, eyes, and the sign it already carries.
- Motion is driven by **the states that already exist** (the four real ones: quiet, asked-not-answered, answered, failed), never by a timer.
- **quiet** — breathing: a very slow, small vertical drift, barely perceptible.
- **asked, not yet answered** — the herd *moves*: a walk/run across its row, staggered per capybara, so it reads as activity rather than a progress bar.
- **answered** — one settled nod, the same gesture a resolved approval gets.
- **needs-you** — the sign goes up and **is never muted by any other state**.
- **`prefers-reduced-motion`** keeps the meaning: state is conveyed by position, opacity and instant change, with no travel.

**Note on the brand.** `BRAND.md` specifies *one character, four poses*. This plan builds to that direction, and the mascot model sheet is still an open commission — if the commission happens, the SVG parts swap in place and nothing else changes.

## B — pricing beside every model

- **Add `price` to the parsed entry.** It is display-only, like `provider` and `link`; the structural test that pins the field set must be **widened deliberately**, with a comment, not loosened.
- **Show it in a form a non-technical person can use.** A raw per-token rate means nothing. Show an **estimate per reply** — *"about $0.0001 a reply"* — and put the basis in the explanation panel that already exists (*"you pay for what you use, by the token"*).
- **Free models say "Free"** — consistent with the label already in use.
- **Unreadable pricing shows nothing**, never a guess. Same rule the balance row already follows.

## C — retest the free models, remove what never answers

The first sweep's criterion was wrong, and this is the fix:

- **Require a real message**, not a status code: HTTP 200 **and** non-empty content **and** not an error string.
- **Repeat each model three times.** Free models are rate-limited by design, so one 429 is not evidence of anything.
- **Classify:** *reliable* (a real message every time) · *flaky* (sometimes) · *broken* (never).
- **Remove the broken.** Keep the flaky but **mark them in the list** — a rate limit is a property of a free tier, not a defect, and silently deleting working models is its own kind of wrong.

**Expected casualties:** `google/lyria-3-clip-preview`, `google/lyria-3-pro-preview` (audio models), plus whatever returns empty across all three attempts.

## D — the truncation, which invalidates every long test until fixed

- **`MAX_REPLY_TOKENS` 256 → 4096.** This is a *product* decision as much as a technical one, because it is the cost ceiling per reply: a longer cap means a longer possible bill. 4096 tokens is a short paper; 256 is two paragraphs.
- **`REPLY_MAX` (8000 chars) must rise with it** — otherwise a long reply gets cut a second time, in the interface, and the bug looks unfixed.
- **The `.reply` block must handle a long answer**: scroll or expand, and no layout blow-out. A long reply arriving in a box designed for two paragraphs is its own defect.
- `PROMPT_MAX = 4000` chars is adequate for now.

## E — the PDF skill, and testing it

Pipeline: **topic → sources → draft → render → save.**

Two decisions still stand, and neither is mine:

1. **Web access.** Research without sources is just generation. Reading the web is a new **declared capability** (D20), a new `docs/threat-model.md` entry, and a gate tier. Requires an explicit yes.
2. **The renderer.** Spike the **WebView print-to-PDF** route first — if it works, the feature costs no dependency at all, which is what the repo's rule demands.

**And this is what makes it a real test of the product:** saving the result is `fs.write` — a **confirm-tier** action — so the card must ask before anything is written. A research-and-write skill is exactly the multi-step case the gate exists for.

**The live stream** is the easy part and belongs with it: named stages (*gathering → reading → drafting → writing*) in a live region, so a two-minute task looks like a task rather than a freeze.

---

## Recommended order

1. **D — the truncation.** Smallest change, and until it's done every long-form test is measuring the cap rather than the model.
2. **B — pricing.** Small, and it's the missing information.
3. **C — the free-model retest.** Needs the corrected criterion; runs once.
4. **A — the capybara characters.** The largest visible win, and independent of the logic — it can go in parallel with the above.
5. **E — the PDF skill.** Blocked on your two decisions.

## What needs you, not me

- **Web access: yes or no.** Everything else in E waits behind it.
- **The reply cap.** How long should an answer be allowed to get? It's a cost ceiling, not a technical limit.
- **Whether the free models that are merely flaky should stay.** I recommend yes, marked.
