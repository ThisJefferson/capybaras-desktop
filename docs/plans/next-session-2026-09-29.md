# Next session — the plan

**2026-09-29.** Supersedes the sequencing in `next-round-2026-09-28.md §6`, most of which has since landed. Every state claim below was checked against the code or the commit log, not remembered.

---

## 1. Where we actually are (verified this session)

**Landed since the last plan was written** — the plan's "still owed" list is now mostly closed:

| Item | State | Evidence |
|---|---|---|
| Pricing beside every model | **Done** | `fe81d91` — `priceLabel` present |
| A–Z ordering | **Done** | `fe81d91` — "the catalogue's own price label and order" |
| The TypeScript mirror | **Done** | `c4a3c96` — "bring the model mirror in line with the catalogue" |
| The interface half (whole reply, never truncated) | **Done** | `97b6510` |
| The capybara characters | **Done** | `81112b0` hand-authored; `b173d00` animates from states; `efdbae8` accent contrast |
| Sidecar bundled for a packaged launch | **Done** | `5d7a133` |
| `innerHTML` banned by T3 | **Holds** | `app.js`: 0 `innerHTML`, 51 `textContent`, 23 `createElement` |

**Still open, and verified open:**

- **Streaming and conversation are absent at the source.** `chat.rs` says so in its own header: *"There is no conversation state, no history, and no streaming."* `request_body(model, prompt, max_tokens)` takes a single prompt. This is the big one.
- **The strict free-model retest** has not been run (200 + a real message, three attempts, keep only what passes every time).
- Uploads, markdown rendering, the PDF skill, and the safe-file model: not started.
- M6 packaging (MSIX, clean install *and* uninstall), M7 signing, M8 hardening: not started.
- **M5's live exit** — one real key, one real call — not done.
- **M4's human step** — a person looking at the real window — not done (this host cannot composite).

---

## 2. The order, and why

### Stage 0 — close the last owed item (small, do first)
**The strict free-model retest.** It is the only piece of the previous round still unrun, and it is a data question, not a feature: 200 *and* a real message, three attempts, keep only what passes every time. Finishing it means the catalogue's free tier is measured rather than assumed.

### Stage 1 — Streaming + conversation (the shape change)
This is the milestone-sized item and it should go next, because three other things are blocked behind it.

**Why it is not optional.** The reply ceiling now asks for the model's own maximum. A maximum-length answer arriving all at once means a long silent wait — the app looks broken precisely in the case the operator asked for.

**What it includes:**
- A persistent thread with scrollback, instead of one reply box replaced each time.
- History sent with each turn (follow-ups that carry context).
- New chat / clear.
- Edit-and-resend, and regenerate.
- **Streaming tokens into the reply as they arrive**, which also gives the herd a live signal for the whole generation instead of only "a call is outstanding".
- **A Stop control** — a long generation with no way to halt it is its own defect.

**The constraint that must be designed in, not discovered:** `chat.rs` warns that a streamed response **never receives a usage block**, so the meter would read zero — calm and wrong. **Whoever adds streaming must ask for usage first.** Whether that is `stream_options: {include_usage: true}`, a closing non-stream call, or a locally counted estimate is the design decision for this stage.

**Done when:** a live reply streams into the window, a Stop halts it, the meter still moves, and a second turn carries the first one's context.

### Stage 2 — Chat parity (the parts that do not touch files)
Markdown rendering by **building DOM nodes, never `innerHTML`** (T4, and T3 is what keeps it safe). Copy, including copy-with-formatting for a long answer. Paste as untrusted text. Enter sends, Shift+Enter newlines, Escape stops.

### Stage 3 — The safe-file model, read *and* write
The threat model gains **five entries**: injection through file content (the highest risk for an agent — a *clean* file can be a working injection), parser exploitation, exfiltration via writes, overwrite/destruction, expansion bombs.

The controls: file content is **data, never instructions**, and never in the decision path; **structural inspection** before opening (what the file *is*, not a signature scan); **parse out of process**, never in the process that holds the credential; the gate covers read, write and open-as-OS-action; **atomic writes** (temp-and-rename); never a silent overwrite; bounds on size and pages; never strip Mark-of-the-Web; and record *why* there is no bundled AV.

**Carry the operator's decision explicitly.** He said *"you can write to any folder you need"* — which **removes containment rather than configuring it.** So the confirm gate on every write, atomicity, and no-silent-overwrite are now the only layers between a mistaken write and the filesystem. That trade must stay visible in the docs, not be quietly absorbed.

### Stage 4 — Uploads (only after Stage 3)
File picker and drag-and-drop, attachments listed with sizes and removable before send. **This is the read path of Stage 3 and must not be built first**, or the inspection and confinement are bypassed by the front door.

### Stage 5 — The PDF skill (after Stage 3)
Write path: topic → sources → draft → render → **save (confirm gate)**. Read path: open → inspect structurally → parse in the sidecar → extract → use as data. **Settle the renderer with a spike first** — try the WebView print-to-PDF route; if it works, the feature costs no new dependency.

### Stage 6 — M6 packaging
MSIX, bundled Node runtime, clean install *and* uninstall. Carries spike-002's two open readings: re-take `app dir writable` against a real `WindowsApps` install, and settle the MSIX state-path question (D15). Note the hard rule already learned: **never resolve paths from `cwd`** (the packaged app launches with `cwd = C:\Windows\system32`).

### Independent of the chain (pick up when a stage is blocked)
- The **free-model mode** and the **D26 spend-cap decision**.
- The **design stream**: typography, Lucide, the motion spec, the mascot model sheet (one character, four poses, one sign).

---

## 3. Process rules for this work

1. **Commit as you go.** Two parallel sessions timed out mid-round; one had finished two of five items and not committed. 838 lines were recovered by hand and one stray `git checkout` from gone. A finished-but-uncommitted stage is not finished.
2. **No subagents.** Direct work only this round.
3. **One stage at a time, verified.** `cargo test` and `npm run verify` must both be green and stated, per the standing habit.
4. **A release per milestone**, with test results — the standing request.

---

## 4. What needs you, not me

| Item | What it needs |
|---|---|
| **M5's live exit** | One real OpenRouter key, one real call — Connect, sign in, Send. The path is built and walked offline; a live key is the only missing piece. |
| **M4's human step** | A person on an **unlocked, interactive desktop** running `run.cmd` and clicking the gate by hand. This host cannot composite a window, so it cannot be done from here. |
| **D26 — the spend cap** | Still open. Nothing enforces a cap today; the app shows the account balance, which is a readout, not a limit. Needs a decision: display-only, or enforced. |
| **Write scope** | Already answered ("any folder") — recorded, with its consequence, in Stage 3. |

---

## 5. The one-paragraph version

The herd, the catalogue and the reply ceiling all landed, so M5's build work is nearly closed and the next real milestone is **streaming and conversation** — a shape change, because the app is currently one question and one answer and has no history at all. That unlocks long replies, gives the herd a live signal, and needs a Stop control and a usage fix so the meter does not read zero. Then chat parity, then the **safe-file model** (read and write, five new threats, structural inspection, parse out of process, atomic writes — with containment knowingly removed), then uploads, then the PDF skill, then packaging. Everything is committed as it lands, nothing is delegated, and the two things that cannot be done from here are a live key and a human looking at a real window.
