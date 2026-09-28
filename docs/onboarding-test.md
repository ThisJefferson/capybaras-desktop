# The onboarding test

**This is M5's exit criterion.** It is the second test that matters, after `docs/acceptance-test.md`.

> A genuinely non-technical person — someone who has never heard the phrase "API key" —
> **reaches a first reply with no help**, and no step requires knowledge they do not have.

If a person can do that, M5 is done. If a step only works because the person already knew
something the product never told them, M5 is not done, whatever the tests say.

---

## What M5 promises, and where the risk actually is

The plumbing of M5 — the OAuth PKCE flow, the credential store, the model catalog, the one
completion — is proven elsewhere:

- `src/onboarding/*.ts` and `tests/{pkce,flow,credential,models,onboarding-flow}.test.ts` — the flow, offline
- `apps/desktop/src-tauri/src/{oauth,loopback,exchange,credential,catalog,chat}.rs` — the shell's half
- `cargo test --test model_call` — the catalog it picks from and the one completion it builds
- `cargo test --test meter` — the meter moving for every finished call, including a failed one

**None of those prove the thing the milestone is about.** They prove parts. The risk M5 carries
is not in a part — it is that the parts are only reachable by someone who already knows how they
fit together. That is what this test walks.

---

## Half one: the automated proof

```powershell
npm run verify:onboarding      # builds the sidecar first
```

`apps/desktop/scripts/verify-onboarding.mjs` drives the **real frontend** — the real
`index.html`, `app.js`, `app.css` and `tokens.css` — in headless Chrome over CDP, with the
**real sidecar** behind it, and asserts what a person would see and could do.

### What is real, and the one substitution

- **Real:** the page, the sidecar, the Chrome, and the product's own path — a click becomes
  `invoke('connect')` / `invoke('send_message')`, which becomes an event, which becomes the DOM.
- **Substituted:** `window.__TAURI__`, because this host cannot display a window
  (`docs/DEBUGGING.md`). That is the same single substitution `verify-acceptance.mjs` makes, and
  it is the same file — `apps/desktop/scripts/harness-bridge.js`, which only moves bytes.
- **Substituted behind that boundary:** the shell's two network calls to OpenRouter (the model
  list, and the completion) are answered from fixtures. The app uses a real key, and the
  instruction is explicit — **no billable call**. Nothing here touches the network or spends
  anything; the live probe stays `#[ignore]`d in `tests/live_api.rs`.

### The assertions

| # | Assertion | What it proves |
|---|---|---|
| 1 | every event the page listens for exists in the shell's source | the two halves cannot drift apart silently |
| 2 | every command the page invokes is registered in the shell | a typo'd command is caught, not shipped |
| 3 | with no key, the panel says so **in plain words** | not a dead end, not an error to interpret |
| 4 | the next step is a **visible, enabled button that names it** | the affordance is signposted |
| 5 | there is **nowhere to paste a key** | the flow never asks for one |
| 6 | no control in the path is unlabelled | nothing exists but is un-signposted |
| 7 | nothing is sent on load | the page does not act on its own |
| 8 | a **failed** sign-in states the reason **in the panel** | a failure is not silent |
| 9 | the way to try again is offered again | a failure is not a dead end |
| 10 | while the sign-in runs, the panel says a browser step is happening | the person knows to look at their browser |
| 11 | Connect is disabled while a sign-in is in flight | no second, confusing sign-in |
| 12 | the model list **loads itself** once connected | reaching a reply does not depend on finding a button |
| 13 | a model is **already chosen**, so nothing must be picked to start | no decision required to begin |
| 14 | the choice is offered **by name**, never by an identifier | no model id the user must already know |
| 15 | a model name carrying markup arrives **as text** | remote text stays text (T3/T4) |
| 16 | the model panel says how many models there are | the state of the list is visible |
| 17 | a reply appears, with no error beside it | **the milestone** |
| 18 | the message was sent with the chosen model, over the product path | a real round trip, not a simulation of one |
| 19 | the meter shows the call it just paid for | what was spent is visible |
| 20 | a refusal reads as a **plain sentence with a next step** | no status code to interpret |
| 21 | Send comes back after a refusal | a refusal is recoverable |
| 22 | no visible onboarding text leaks `undefined`/`NaN`/`[object Object]`/`HTTP` | nothing on screen is a machine artifact |
| 23 | the gate **still halts** a destructive action | onboarding did not disturb the gate |
| 24 | the harness never met a command it did not implement | the check covers the whole surface it drove |

### The two defects this found — both real, both fixed

1. **A failed sign-in said nothing on screen.** `capybaras://connect-failed` wrote its reason to
   the developer log and re-read the connection state, which put "Not connected yet." back. A
   person whose sign-in timed out saw a button that appeared to have done nothing, with the
   reason somewhere they never look. Fixed: the reason is now written to the connection panel
   itself, in the words the shell already produces.
2. **During the sign-in, the panel said "Not connected yet."** The only sign the sign-in was
   running was a log line. A person sent to their browser and back had no confirmation on the
   panel that anything was happening. Fixed: `capybaras://connect-waiting` now says a browser
   window has opened and to come back.

Both were found by **asserting the thing a person needs**, failing, and fixing the product —
not by loosening the assertion.

---

## Half two: driving it yourself

This is the part that proves the *product* rather than the interface.

1. Double-click **`run.cmd`** in the repository root. (First run compiles Rust — several minutes.)
2. A window opens. In **Your model**, click **Connect to OpenRouter**.
3. A browser window opens. Sign in to OpenRouter and click **Authorize**. Return to Capybaras.
4. **The model list fills itself in**, with a model already chosen. You do not have to pick one.
5. Type anything in **Your message** and click **Send**.

### What you should see

- **Before connecting:** "Not connected yet." and a **Connect to OpenRouter** button. Nothing
  asks you for a key.
- **While signing in:** the panel says a browser window has opened.
- **After connecting:** "Connected.", and **"N models available."** with a model already selected
  by name.
- **After Send:** the reply, and **This session** counting the call.

### What must be true

- **No key is ever pasted.** The sign-in is OAuth; the key is kept by Windows, not in a file.
- **No model identifier is ever needed.** Models are offered by name, and one is pre-chosen.
- **No error has to be interpreted.** Every failure is a sentence with a next step.
- **Reaching a reply never depends on discovering a button.** Connecting loads the list.

## What failure looks like

- the panel is blank, or says only "Error";
- the user must paste a key, or type a model identifier;
- a failed sign-in changes nothing on screen;
- the model list stays empty until a button nobody mentioned is found;
- a reply never appears, or the reply only appears after something is reloaded;
- a status code, a stack trace, `undefined` or `[object Object]` appears anywhere on screen.

---

## Known gaps, stated rather than hidden

- **The seam between the page and the shell is asserted, not executed.** The frontend reaches the
  shell by *name* — command strings and event strings. This test asserts that every event the page
  listens for exists in the shell's source, and every command it invokes is registered. It does
  **not** execute the Rust path: the shell's own behaviour is proven by the Rust suite, and the
  **payload field names** (`models`/`default`, `text`/`model`) are the one part neither side
  re-derives from the other. A rename there would still pass this check. Closing it properly means
  exercising the real shell, which needs the window this host cannot show.
- **The window.** As with the acceptance test, `npm run verify:ui` and `verify:onboarding` are
  green, but no human has looked at the running Tauri window. `docs/DEBUGGING.md` records why this
  host cannot.
- **A first reply against the real OpenRouter account.** Not run here, by instruction — it spends
  money. Either run `cargo test --test live_api -- --ignored --nocapture` in
  `apps/desktop/src-tauri`, or click through by hand on an unlocked desktop.
- **The spending cap is not an enforced control in this app.** See
  `docs/plans/M5-onboarding.md` §4 and the note below.

## The spending cap

M5's plan lists a "spend cap". **There is no enforced cap in the app** — the meter counts and
displays, and nothing refuses a call on the basis of what has been spent. That is deliberate in
the design (`docs/plans/M5-onboarding.md` §4: *a local cap is advice; a provider-side cap is a
limit*), but it is currently **only half-built**: the app neither enforces a cap nor shows the
provider-side limit that does. This is recorded as a gap rather than papered over — see the
status note in `docs/plans/M5-onboarding.md` §4.
