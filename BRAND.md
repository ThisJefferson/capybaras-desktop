# Capybaras — Brand & Interface Identity

**A herd of calm helpers that ask before they act.**

Compiled 2026-09-27. Companion to `BUILD-PLAN.md` (Part II) and `research/safe-agent-blueprint.md`.

---

## 1. The name

**Capybaras.** Plural. That is the whole concept in one word.

### The name collision — checked, and it matters

**`Capybara` (singular) is an established software name.** `teamcapybara/capybara` is a widely used MIT-licensed Ruby acceptance-testing framework — the `capybara` gem, with its own Wikipedia page, in every Rails developer's bundle.

Practical consequences:

- **The plural is distinct enough for a product wordmark** — and it *is* the concept, so the plural is not a workaround, it is the point
- **Avoid the bare slug `capybara`** for the repo, the npm package, or the CLI binary — that is the collision
- Use `capybaras` or `capybaras-desktop` for repo and package names
- **Check availability before committing:** npm, PyPI, GitHub org, the .com/.app domain
- Keep the mark visually distinctive enough that nobody confuses a desktop agent with a test framework. They will not.

**Trademark position:** the practical risk here is low. Two different software categories, a plural-vs-singular difference, and a completely different visual identity. But do a proper clearance search before spending money on a domain, and never claim the word — just the wordmark.

---

## 2. Why capybaras are the right mascot — and it is not arbitrary

Three reasons, and the third is the one that makes this a *good* design decision rather than a cute one.

**1. Temperament matches the promise.** Capybaras are famously unbothered. They are the animal that other animals sit on — birds, monkeys, turtles, whatever wanders past. Nothing rattles them. **That is exactly the personality a safety product needs.** A security warning delivered by something that is visibly calm lands as reassurance; the same warning delivered by an alarmed mascot lands as panic. The capybara makes "we are being careful" feel soothing instead of scary.

**2. They are genuinely Brazilian.** The capybara is native to South America and abundant in Brazil — it is not a decorative import. The Rio setting is authentic to the animal, not a costume on it.

**3. The herd = the architecture.** This is the important one.

The product runs **multiple agents**. OpenClaw's real design has a main agent plus specialist workers — one for research, one for checking, one for web lookups, one for editing, one for organising. Today that is **invisible**: the user sees a single chat box and has no idea that four things are happening.

**A herd of capybaras makes the architecture legible.**

> Five capybaras dozing, two awake, one walking up front with a sign — and the user instantly understands: *something is being worked on, by specialists, and one of them needs me.*

That is not decoration. **It is the best available interface for a multi-agent system**, and it happens to be adorable. TunnelBear had one bear for one thing. We have a cast, because we have a cast.

---

## 3. The cast

Names are warm, short, Brazilian, and easy to say in English. Each capybara has **one job**, so the user learns who does what.

| Capybara | Job | Maps to | Wakes when |
|---|---|---|---|
| **Tuca** | **The one you talk to.** Speaks for the herd. | Main agent | Immer — always present at the front |
| **Bia** | Looks things up; reads pages | web_lookup / research | Research or browsing is needed |
| **Zeca** | **Checks the work.** The sceptic. | checker / verification | Something must be double-checked |
| **Nina** | Writes and edits files | edit / write | Documents are being created or changed |
| **Joca** | Runs things; handles the machine | tools / exec | A command or system action is needed |
| **Duda** | Tidies up; organises the results | normaliser / batch | Collecting, sorting, summarising |

**Narrative rule: Zeca is the one who asks.** The verification agent is the one that stops and raises a paw. That is the product promise, embodied — *the sceptic is a member of the team, not a gate bolted on.*

Six is the right number: enough to feel like a herd, few enough to name.

---

## 4. The four states

The TunnelBear move — make the security/status state legible through a character — but multiplied, so it also shows *who*.

| State | What you see | Motion |
|---|---|---|
| **Dozing** | The whole herd in a pile, chests rising slowly | Very slow breathing, 4–6 s cycle |
| **Listening** | Heads up, ears forward, eyes open | Gentle sway, occasional blink |
| **Working** | Only the relevant capybara(s) awake and busy; the rest still dozing | Purposeful, syncopated — bossa rhythm |
| **Needs you** | **One capybara walks to the front holding a small sign** | **The only state with asymmetric motion — it must be unmistakable from across the room** |

Rules:
- **"Needs you" is the loudest thing in the entire app.** Not a badge, not a red dot — a character standing in front of you.
- **Never make more than two capybaras look busy at once**, or the interface reads as chaos.
- The herd is **always visible**, even when everything is idle. Its stillness is informative: *nothing is happening, and that is a fact you can see.*

---

## 5. The Rio visual language

### What to take from TunnelBear

Take the **approach**, never the surface:
- One obvious primary action
- Plain-language state names
- A mascot that carries emotional meaning
- Zero configuration for the normal case
- Immediate, visible feedback

**Explicitly do NOT take:** their grey/blue palette, their brown-bear illustration style, their layout, their type choices, their iconography. Different animal, different country, different feeling.

### The calçadão — the anchor motif

The **Copacabana boardwalk wave**: black and white Portuguese pavement, laid along Avenida Atlântica in 1905–06 by Portuguese *calceteiros* under Mayor Pereira Passos, black basalt and white limestone, waves running perpendicular to the sea. Ipanema got its own geometric pattern afterward.

This is a **public pavement design for a place** — an iconic, widely referenced Rio motif, not any company's mark. Use it as:

- A **subtle background texture** at very low opacity (it should whisper)
- The **loading and transition motif** — the wave animating as progress
- A thin **brand strip** in the header

**Do not** reproduce it as a decorative sticker in the corner. It should feel like the ground the app stands on.

### Palette

Deliberately warm where TunnelBear is cool. Rio, not Zurich.

| Token | Hex | Use |
|---|---|---|
| `sand` | `#FBF7F0` | Base background — Copacabana sand |
| `atlantic` | `#0F5C63` | Primary brand, wordmark, primary buttons |
| `coral` | `#F2705A` | Accent — **reserved for "needs you"** |
| `capy` | `#8B6239` | Mascot body |
| `leaf` | `#4E8C5A` | Secondary accent, success |
| `ink` | `#161616` | Body text (the black of the pavement) |
| `sky` | `#CFE7E6` | Surfaces, cards, dividers |

**Coral is reserved.** It means one thing: *a capybara is waiting for you.* Nothing else in the app may use it.

**Avoid the Brazilian flag palette** — green/yellow/blue in that combination reads as cheap and nationalistic rather than warm. The Rio feeling lives in the pavement, the light, and the greens, not the flag.

### Typography

- **Rounded, friendly, geometric sans** for the wordmark and headings — warm, not corporate
- **System font stack for body** (Segoe UI Variable on Windows 11) — fast, native, readable
- **16 px minimum body text.** Larger default than usual; the audience is non-technical and often older
- Generous line height (1.6), short line lengths, no dense paragraphs

### Motion — the bossa nova rule

Bossa nova is gentle, syncopated, and unhurried. That is the motion brief.

- **150–250 ms, ease-out.** Nothing snaps.
- **Sway, don't bounce.** Capybaras move like water, not like a spring toy.
- **Syncopation over simultaneity** — the herd should not move in unison. Two capybaras doing the same thing at the same instant looks mechanical.
- **Always respect `prefers-reduced-motion`.** Required, not optional.

---

## 6. Tone of voice

Warm, plain, calm, unhurried. The capybaras never sound alarmed, and never sound corporate.

**Rules:**
- Plain English. Never a technical term. (See the banned-words list in `BUILD-PLAN.md` §14.)
- **Short sentences.** A nervous user reads less than you think.
- **Never blame the user.** "That didn't work" — not "You entered an invalid input."
- **Warm, but not twee.** No "Oopsie!" and no exclamation marks in a warning.
- Portuguese is **seasoning, not the language.** A greeting, a tagline, a state name in the About screen. The interface is English by default, with pt-BR available as a language.

### The copy deck

**Wordmark / tagline:**
- *Capybaras*
- *The agents that ask first.*
- *(alt) A calm herd that helps. Nothing happens without a nod.*

**Empty state:**
> All quiet. The capybaras are dozing.

**Listening:**
> Tuca is listening.

**Working:**
> Zeca is checking that now.

**Needs you — the approval card:**
> **Tuca wants to delete 3 files from Documents.**
> This can't be undone.
> *Why: you asked me to clear out old drafts.*
> `[ Hold on ]` ← default · `[ Go ahead ]`

**Money, always visible:**
> This month: **$3.41** of $10.

**Errors — and this is the important one:**
> That didn't work. **Nothing was changed.**

**Every error message states whether anything changed.** For a product whose entire promise is "the worst case is boring," that one sentence is the most reassuring thing in the interface, and almost nobody writes it.

**Approval buttons:** `Hold on` / `Go ahead` — warmer than Cancel/OK, and unambiguous. **`Hold on` is the default, and Enter must never trigger `Go ahead`.**

---

## 7. Fun — where to spend it, and where not to

Fun is a budget. Spend it on moments that reinforce the promise; never on moments that obscure it.

**Spend it here:**
- **The pile.** When everything is idle for a while, the capybaras pile up. Tap them and they resettle. Purely delightful, zero information cost.
- **The yawn.** A capybara yawns on wake. Small, occasional, not every time.
- **The bird.** Birds ride on capybaras. During a long task, a little bird lands on the working capybara. It is a progress indicator that people will screenshot.
- **The nod.** When a task completes, a single satisfied nod. No confetti, no sound unless enabled.
- **The wave.** The calçadão motif animates as the transition between screens.
- **The sign.** The "needs you" capybara holds a small hand-lettered sign. The sign is where personality goes — it is also the one moment you can be funny, because the approval card underneath is serious.

**Never spend it here:**
- **The approval card.** No jokes, no mascot mugging, no playful copy. This is the moment that must be read carefully. The capybara may be present, calm, and quiet — it may not be a distraction.
- Anything destructive. Nothing cute attached to a delete button.
- Numbers, money, or permissions. Precision, not personality.

**Optional sound:** a soft marimba chime when a capybara needs you. **Off by default**, offered once during setup, never required, never for errors.

---

## 8. Cultural care

The Rio setting should read as **specific and affectionate**, not as a costume.

**Do:**
- Reference the actual place — the calçadão, the light, the Atlantic greens, bossa nova's rhythm, the animal that genuinely lives there
- Use real Portuguese correctly (get a native speaker to check every string — *Capivaras* is the correct plural)
- Offer pt-BR as a proper interface language, not a gimmick
- Keep it understated. Confidence is quiet.

**Don't:**
- Carnival, feathers, maracas, samba stereotypes, or the flag as decoration
- Any "tropical" cliché that would not survive a Rio resident's glance
- Accented caricature in the voice acting, if voice is ever added

**Test:** if a person from Rio would smile and say "that's nice," it works. If they'd wince, it does not.

---

## 9. One-page summary

| Element | Decision |
|---|---|
| **Name** | **Capybaras** (plural, always) |
| **Repo / package slug** | `capybaras` or `capybaras-desktop` — never bare `capybara` |
| **Concept** | A herd of calm helpers that ask before they act |
| **Mascot system** | 6 named capybaras, each with one job — makes the multi-agent architecture visible |
| **States** | Dozing · Listening · Working · **Needs you** |
| **Anchor motif** | Copacabana calçadão wave pattern |
| **Palette** | Sand, Atlantic teal, capybara brown, leaf — **coral reserved for "needs you"** |
| **Motion** | Bossa nova: 150–250 ms, ease-out, sway not bounce, syncopated |
| **Voice** | Warm, plain, calm, never alarmed, never corporate |
| **Fun budget** | The pile, the yawn, the bird, the nod — never on the approval card |
| **The promise** | It will still break. It will break small, visibly, and you can undo it. |

---

## Sources & references

- `teamcapybara/capybara` — established MIT Ruby acceptance-testing framework (name-collision check); Wikipedia entry *Capybara (software)*
- Copacabana calçadão — Portuguese pavement laid along Avenida Atlântica, 1905–06, under Mayor Pereira Passos; black basalt and white limestone in a wave pattern, perpendicular to the sea; Ipanema's promenade adopted a different geometric pattern afterward
- `BUILD-PLAN.md` §14 — the performance and interface specification this document dresses
- `research/safe-agent-blueprint.md` — the TunnelBear UX translation this builds on

*Method note: the name collision and the calçadão history were verified by search rather than assumed. The cast, palette, and copy deck are design proposals — opinions, offered to be argued with. The cultural guidance in §8 is the part most worth having a Brazilian review before launch.*
