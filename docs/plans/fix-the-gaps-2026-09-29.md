# Fixing the seven gaps
## A plan, sequenced by what unblocks what

**Compiled 2026-09-29.** Follows `design-comparison-2026-09-29.md`, which found seven things we are missing or overlooking. This is how they get fixed.

---

## 0. The shape of the plan, and the one insight that orders it

**The seven gaps split into two tracks, and they have different clocks.**

| track | what it is | what it needs |
|---|---|---|
| **A — Security** | Stage 3.3–3.5 of the safe-file model | **Nothing but time and tests.** No user, no decision. |
| **B — Design** | Gaps 7.1–7.7 | **Needs a user we cannot currently reach.** |

**The insight that orders everything:** **Track A can run now, and Track B cannot be validated until someone who is not us can install the app.** Every card we design is untested against its audience, and no amount of further design work fixes that. **So the installer is not one task among seven — it is the gate to the other six.**

**But it is also the expensive one** (bundling, signing, a decision from Jeff). So the plan does the cheap design work first, runs Security in parallel, and treats the installer as the milestone that unlocks real learning.

---

## 1. Track A — Security, unchanged and uninterrupted

**No dependencies on any of this. It proceeds exactly as already sequenced.**

1. **3.3 — Atomic writes.** Temp-and-rename, never a silent overwrite. Small; finishes T15.
2. **3.4 — Out-of-process parse.** The sidecar parses; the shell keeps the key. Closes T13's second half.
3. **3.5 — Bounds during parsing.** The inspector enforces bounds at inspection time; this enforces them *while* a document is read. Closes T16 fully.
4. **Then, and only then:** uploads, then the PDF skill.

**The ordering rule still binds: no uploads until 3.4 exists.** The inspection and the gate are built; the parse path behind them is not.

**Estimate:** 3.3 is one session. 3.4 is the largest single piece left. 3.5 is small once 3.4 exists.

---

## 2. Track B — The seven fixes

Each has: what breaks, the fix, effort, dependency, and **done-when** — so nobody has to argue about whether it is finished.

### Wave 1 — The card gets honest, and the herd learns to bow out *(no dependencies, days)*

These three are cheap, they improve the moment that matters most, and none needs the installer.

#### 7.7 — Help where the person is
**Fix.** One plain-language line per hazard, **in the card**, beside the word that needs it. Then a small glossary behind a *"what does this mean?"* link on the card itself.
**Why the card and not a help page.** Because the confusion happens *at the card*. A person who has to leave the decision to look something up will simply click.
**Effort:** small. **Done when:** every hazard the classifier can raise has a one-line explanation reachable from the card without leaving it.
**Note:** the classifier's `reasons` are already plain-language — *"This cannot be undone."* The gap is the *words around* them (reversible, taint, hard gate), not the reasons themselves.

#### 7.3 — Put our honesty where a person can see it
**Fix.** A short in-app page: **what this can do · what it cannot do · what it will always ask.** Built from the threat model's own *"what we deliberately do not defend against"* section — which is more honest than anything TunnelBear publishes, and currently sits in a file no user will open.
**Effort:** small. Mostly writing, which is already done — it needs adapting, not inventing.
**Done when:** a non-technical reader can answer *"should I trust this?"* from inside the app, in under a minute.

#### 7.5 — The rule for when the herd gets out of the way
**Fix.** Write the decision down, and enforce it:

> **The permission card is a mascot-free zone.** Charm everywhere else.

**Why here specifically.** It is the one screen where the product's entire promise is exercised. A permission card is where a person decides, and a mascot beside a decision either distracts or — worse — *looks like it is endorsing the answer*.
**Effort:** small — a decision plus a line in the design docs and a test that no mascot renders on a card.
**Done when:** the rule is in the design docs, and something fails if it is broken.

### Wave 2 — Remove the technical decision *(small dependency)*

#### 7.2 — Take the model choice out of the happy path
**Fix.** **Default to one model.** If a choice exists at all, it is *"Free"* and *"Best"* — never a model name, never a context length.
**Dependency:** the **free-model mode**, which is already scoped and whose data already exists (the retest found 4 reliable free models). This is where that work pays off — it was filed as a *cost* feature; it is really a *simplicity* feature.
**Effort:** medium (catalogue logic + one UI change).
**Done when:** a person who does not know what a model is can complete onboarding without meeting one.

### Wave 3 — The gate to learning anything *(needs Jeff)*

#### 7.1a — The installer
**Fix.** Tauri bundling (NSIS or MSI), so the app installs like software rather than compiles like a project.
**Effort:** medium. **Needs a decision from Jeff:** code-signing certificate or not. Unsigned installers trigger a Windows warning that a non-technical user will stop at — which would defeat the entire point of this wave.
**Done when:** a person with no developer tools double-clicks one file and reaches the Connect screen.

#### 7.1b — Three people who are not us
**Fix.** Watch **three genuinely non-technical people** install and use it, without helping. The onboarding walk already exists (`docs/onboarding-test.md`) — it needs to be *run on humans* rather than in a harness.
**Depends on:** 7.1a. **Needs Jeff:** the people.
**Done when:** we have seen three people reach a first reply, and written down where each of them hesitated.

**This is the single most valuable item in the plan.** Everything in Track B before it is a guess; everything after it is informed.

### Wave 4 — Informed by what the testers did

#### 7.4 — Make the herd the status
**Fix.** Audit every spinner, progress line and status word. For each: **should the herd be this instead?** One status display, not two.
**Deliberately after 7.1b**, because *which* states matter is exactly what watching people will teach us.
**Done when:** there is one place to look to know what is happening, and it is the herd.

#### 7.6 — What happens after it is wrong
**Fix.** A *"what's new"* surface, and an **update check that notifies and never self-installs**.
**Why notify-only:** OpenClaw's rule — we do not self-update — and our own promise is that things break *visibly*. An app that quietly replaces itself contradicts the promise it is built on.
**Done when:** a person can see what changed and choose when.

---

## 3. The sequence, laid out

```
NOW       Track A: 3.3 ──► 3.4 ──► 3.5 ──► uploads ──► PDF skill
          Track B: 7.7 · 7.3 · 7.5        (days, no dependencies)
                              │
                     [ Jeff decides: signing ]
                              ▼
          Track B: 7.1a installer ──► 7.1b THREE HUMANS
                                          │
                                          ▼
                     Track B: 7.4 · 7.6   (informed by what we saw)
          Track B: 7.2 (rides the free-model mode, any time after Wave 1)
```

**Two tracks, one rule:** Track A never waits on Track B, and Track B never ships an untested design change into the card once testers exist.

---

## 4. What this plan needs from Jeff

| # | decision | blocks |
|---|---|---|
| 1 | **Code-signing certificate — yes or no?** | 7.1a, and therefore 7.1b, and therefore all informed design work |
| 2 | **Three non-technical people.** | 7.1b |
| 3 | **A live OpenRouter key** — still the only thing between M5 and done | the first real reply |
| 4 | **The D26 spend-cap decision** — open since M5 | not this plan, but still open |

**Item 1 is the one with a cost attached**, and it is the one I would resolve first, because it gates the most.

---

## 5. What I would do first

**Start both tracks today, in this order:**

1. **7.5 — write the mascot rule.** It costs a paragraph and it prevents a class of mistake for the rest of the project.
2. **7.3 — the honesty page.** The material is already written and unusually good; it only needs to be moved where a person can see it.
3. **7.7 — the card glossary.** Small, and it improves the one interaction the product is *about*.
4. **3.3 — atomic writes**, so Track A keeps moving.

**Then stop and ask about signing**, because until that is answered, no design decision we make can be checked against a real user.

---

## 6. What this plan deliberately does not do

- **It does not touch the mascot art.** The design doc's finding stands: beauty is mostly typography, spacing, colour and motion, and the interface must not depend on the art being excellent. Hiring an illustrator is not on the critical path.
- **It does not add features.** Every item above is either a rule, a re-wording, or a removal of a decision. **Nothing in this plan makes the product do more.**
- **It does not promise the observation will be comfortable.** The likeliest outcome of 7.1b is that the card is wrong in a way we cannot currently see.

<!-- project: path:C:\Users\Skept\.openclaw\workspace -->
