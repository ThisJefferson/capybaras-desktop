# Spike 003 — Can we just generate the mascots?

## Verdict: PARTIAL

**Question:** capybaras are integral to this product, so the art has to be good. Can OpenRouter image generation produce the mascots, instead of commissioning illustration?

**Short answer: generation is genuinely excellent for *exploring* the design, and not good enough for *shipping* it.** The gap is not resolution or rendering quality — it is **character consistency**, which is the one property a mascot set cannot do without.

---

## Method

Three generations via `google/gemini-3-pro-image`, ~20 s and **$0.14 each**:

| File | Subject |
|---|---|
| `zeca-01.png` | Zeca, holding a stop sign |
| `zeca-02.png` | Zeca — *the identical prompt again*, to test consistency |
| `tuca-01.png` | Tuca — same style, different character |

The workspace's usual vision script is prompted for phone screenshots and answered the wrong question, so a purpose-written evaluator was used (`evaluate.ps1`), asking for literal description, 64-pixel legibility, craft, and a comparison across images.

---

## What the evaluation found

**Generation works, and it is cheap and fast.** That part is settled — $0.14 and twenty seconds per image, no new infrastructure, no account to create.

**But the output is not shippable as a mascot set.**  The specific defects:

- **Zeca's sign sits inside his torso silhouette**, so at 64 px the prop, the hand and the belly patch collapse into one brown-red blob. **Zero silhouette separation** — the single most basic requirement for an icon.
- **Malformed hands.** "An awkward cluster of four indistinct, melting sausage-digits." Props are the first thing generative vector-style art fails at.
- **Random low-resolution debris** — a deformed folder icon, a wonky cursor, stray floating dots. At icon size these become one-pixel noise.
- **Muddy internal detail** — Tuca's badge and lanyard are gibberish, and the neck loop does not drape.
- **An unfocused, faintly hostile expression** — from eyebrow slashes nobody asked for.

**And the decisive finding: the two Zeca images are not the same character.** Same prompt, same text, and:

| | `zeca-01` | `zeca-02` |
|---|---|---|
| Muzzle | enclosed outlined block with nostrils | no outline, nostrils on a flat face |
| Head | oval, ears tilted out | bottom-heavy pear, upright ears |
| Belly patch | large, low-contrast, dark tan | small, high-contrast, cream |
| Outline | thick teal-slate | thin warm charcoal |
| Extras | none | floor shadow |

That is **continuity drift**, and it is structural. A herd of six characters across four states needs **24 mutually consistent poses**. If two runs of the same prompt produce two different capybaras, the other 22 are not reachable this way.

**Art-director verdict, in its words:** *"Send them back immediately. Hire a vector illustrator to draw a single, clean, standardized character model sheet."*

**Caveat on the evidence:** the evaluator is a vision model judging generated art — an AI assessing AI. Its criticisms are specific and checkable rather than vague, which is why they were kept, but a human art director should confirm before any money is spent.

---

## Recommendation

**Use generation for the part it is actually good at, and buy the part it cannot do.**

1. **Generate to explore, not to ship.** At $0.14 an image, we can produce dozens of variations and converge on a *direction* — silhouette, warmth, how simple, how serious. That is a real, cheap, fast capability and it should be used. It replaces mood boards, not illustrators.
2. **The thing to commission is a character model sheet, not 24 illustrations.** One canonical drawing per character — front, side, expression range, the prop — is the actual asset. It is a much smaller job than a full illustration set, and once it exists, the states can be *derived* from it rather than re-imagined.
3. **The model sheet also fixes the consistency problem.** With a canonical reference, image-to-image generation becomes viable for derivative poses, because the model is matching a reference rather than inventing a character from a prompt each time.

**Sequencing:** explore with generation now (free-ish, and it sharpens the brief), commission the sheet when there is budget, derive the rest.

---

## Cost reality

- Generation: **~$0.14 per image.** A hundred-image exploration pass costs about **$14** — genuinely cheap, and worth doing before writing any brief.
- Commission: the real cost, and the one decision that needs Jeff. A model sheet for six characters is a smaller, more standard job than a full character set, so it should be quotable.

---

## Files

- `generate.ps1` — OpenRouter image generation (key from the environment, never printed)
- `evaluate.ps1` — vision evaluation with a task-specific prompt
- `zeca-01.png`, `zeca-02.png`, `tuca-01.png` — the evidence

**Reproduce:** `& .\generate.ps1 -Prompt "<text>" -Out "name.png"`. Note: pass **absolute** paths — .NET resolves relative paths against the process directory, not PowerShell's location.
