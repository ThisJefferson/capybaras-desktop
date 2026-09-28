# The next round — chat parity, safe files (read *and* write), the PDF skill

**2026-09-28.** Supersedes the sequencing in `five-changes-2026-09-28.md`, which is now half-done. **Every claim below was checked against the code or a run.**

---

## 1. What landed

Committed as `0197c35`, verified with `cargo test` **and** `npm run verify`, both exit 0:

- **The reply ceiling is fixed.** `chat.rs` asked for `max_tokens: 256` — two paragraphs, which is why the research paper came back cut off. It now asks for **the model's own ceiling** (`top_provider.max_completion_tokens`, carried as `maxCompletionTokens`), with a deliberately high fallback when the catalogue does not say. An unknown ceiling must never be read as a short one.
- **A rule for entries that cannot answer a text chat.** `google/lyria-3-*` music models were being offered among the free ones. Any entry advertising a non-text output modality is now dropped — **15 of 458 measured**. A rule, not a list, so it survives the catalogue churning.

## 2. Still owed from that round

| Item | State |
|---|---|
| Pricing per model | **Not done** — `priceLabel` appears nowhere. |
| A–Z sorting | **Not done.** |
| The strict free-model retest | **Not run** — 200 **and** a real message, three attempts, keep only what passes every time. |
| The TypeScript mirror (`src/onboarding/models.ts`) | **Not updated** — Rust moved, the mirror did not. |
| The interface half — the whole reply, the price, the order | **Not started.** |
| The capybara characters | **Not started.** |

---

## 3. Chat parity — and this is a shape change, not a feature list

> *"all of the features in a regular chat agent … uploading a document and copy and pasting"*

**The app today is one question, one answer.** A regular chat agent is a **conversation**. That is not an addition to the current screen; it changes what the screen is. Better to say so now than to discover it halfway through.

**The conversation itself**
- A thread that persists as you talk, with scrollback, instead of a single reply box that is replaced each time.
- Follow-up questions that carry context — which means the app has to send history, not just the latest message.
- New chat / clear, so a session can start clean.
- Edit and resend, and regenerate — the two things people reach for when an answer is close but wrong.

**Streaming, and why it stops being optional**
Raising the reply ceiling to the model's own maximum creates a new problem: **a very long answer arriving all at once means a very long silent wait.** Without streaming, the app looks frozen for the exact case the operator asked for. So:
- Stream tokens into the reply as they arrive.
- **This also gives the herd a live signal for the whole generation**, rather than only knowing that a call is outstanding — the animation and the answer would move together, which is what he asked to see.
- Streaming needs a **stop** control. A long generation with no way to halt it is its own defect.

**Documents and the clipboard**
- **Upload a document** — file picker and drag-and-drop. **This is the read path of the file model (§4) and must not be built before it**, or the inspection and confinement are bypassed by the front door.
- **Attachments shown as a list**, with sizes and a way to remove one before sending.
- **Paste** — text into the input; pasted content is untrusted text like any other input.
- **Copy** — a reply, and copy-with-formatting for a long one, because a research paper you cannot copy out of is not finished.

**Rendering**
- **Markdown.** A long answer arrives as markdown and today would render as literal asterisks. Rendering it must be done by **building DOM nodes**, never `innerHTML` — remote text in a trusted position is threat T4 and the `textContent`-only rule (T3) is what keeps it safe. This is a real design constraint, not a detail.
- Code blocks, lists, headings, tables — readable without a stylesheet fight.

**Keyboard, since a chat window is used from the keyboard**
- Enter sends, Shift+Enter makes a newline.
- Escape stops a generation in flight; focus behaviour stays consistent with the overlay conventions already established.

## 4. Safe files — read *and* write

**Scope settled by the operator: both.** That puts the ingestion path in scope and makes this real engineering.

### Threat model gains five entries
**Injection through file content** — a file whose *contents are instructions*. **The highest risk for an agent**, and the README already concedes prompt injection may be permanent rather than fixable. An antivirus does nothing about it: a clean file can still be a working injection. · **Parser exploitation** — malformed PDFs, embedded JS, `/Launch`, `/EmbeddedFile`. · **Exfiltration via writes.** · **Overwrite and destruction.** · **Expansion bombs.**

### The controls
1. **File content is data and can never become instructions.** Quoted material, never in the decision path. **A file cannot tell the app what to do.** This is architectural, not a filter.
2. **Structural inspection before opening — not a virus scan.** What the file *is*: embedded JavaScript, launch actions, embedded executables, macros, encryption, and whether the declared type matches the extension. No signature feed, nothing to go stale.
3. **Parse out of process, in the least privileged place.** The sidecar already runs beside untrusted content; the shell holds the key (T12). **A file must never be parsed in the process that owns the credential.**
4. **The gate covers file actions**: read, write, and open-as-an-OS-action. Writing is `confirm`; handing a file to the OS is `confirm`; deletion stays a hard gate.
5. **Writes are confined**: one declared folder, no traversal, **atomic writes** (the temp-and-rename pattern grants already use), **never a silent overwrite**.
6. **Bounds** on size, pages and extracted text.
7. **Respect the machine's own protection** — never strip Mark-of-the-Web, never route around SmartScreen, never auto-open.
8. **Record why there is no bundled antivirus.** "We deliberately do not scan, and here is what we do instead" is defensible; "we scan files" without saying what that means is not.

## 5. The PDF skill and the live stream

**Write:** topic → sources (web access approved) → draft → render → **save (confirm gate)**.
**Read:** open → **inspect structurally** → parse in the sidecar → extract → use as *data* → answer about it.
**Renderer:** settle it with a **spike** — try the WebView print-to-PDF route; if it works the feature costs no dependency.
**The stream:** named stages (*gathering → reading → drafting → writing*) in a live region, so a two-minute task looks like a task.

---

## 6. Sequence

1. **Finish the last round** — pricing, A–Z, the free retest, the TypeScript mirror, the interface half. *(The interface cannot start until the mirror and the price field agree.)*
2. **The capybara characters** — biggest visible win, untouched, independent of everything else.
3. **Streaming + conversation** — the shape change, and it makes long replies usable.
4. **Chat parity** — uploads, paste, copy, markdown. **Uploads wait for §4.**
5. **The safe-file model** — needs one decision first (below).
6. **The PDF skill** — depends on §5's controls, because reading a file needs them.
7. **The installer** — still what makes this handable to anyone else.

## 7. What needs the operator, not me

- **Which folder may the app write to?** All write confinement follows from this.
- **Should reading a file ask every time, or can a folder be granted once?**
- **Confirmation that no bundled antivirus is the accepted position** — with structural inspection and data-not-instructions as what replaces it.

## 8. Process note

Two sessions ran in parallel for twenty minutes and **both timed out**, one having finished two of five items **without committing**. 838 lines were recovered by hand from the working tree — one stray `git checkout` from gone.

**The lesson is not "don't parallelise"; it is "commit as you go".** A stage that is finished and uncommitted is not a stage that is finished.
