# M5 — onboarding, and the one decision that matters most

**Status:** started. PKCE is implemented and tested; nothing is connected to a network yet.
**Design decision:** D7 — two-click setup, no key pasting.
**Blocked on:** an OpenRouter OAuth app registration (client ID) from Jeff.

---

## 1. The goal, stated as a constraint

> **From install to a working agent in two clicks, by someone who has never heard
> the phrase "API key".**

That is the requirement, not a nicety. The product's whole claim is that it makes
an agent safe for someone who cannot audit one, and that audience does not belong
in a provider's settings page copying a bearer token. D7 settled this: OAuth, no
key pasting.

---

## 2. The flow

1. The user clicks **Connect**.
2. The app generates a PKCE verifier and state (done — `src/onboarding/pkce.ts`),
   opens the system browser to the provider's authorization URL, and starts a
   **loopback listener on 127.0.0.1**.
3. The user signs in and approves. The provider redirects to the loopback address
   with a code.
4. The app checks the returned `state` against the one it sent, exchanges the code
   plus the **verifier** for a token, and stores the token.
5. The window shows "Connected" and the monthly cap.

**Why PKCE and not a client secret.** A desktop application cannot keep a secret.
Anything in the binary is readable by whoever has the binary; anything on disk is
readable by the account the app runs as — which, as the threat model keeps
pointing out, is the same account the agent runs as. PKCE needs no secret: the
verifier never leaves the machine, so an intercepted code is useless.

**Why the system browser and not a webview.** A sign-in page inside our own
webview means the user is typing credentials into a window our code controls, and
the app could read them even if it does not. The system browser keeps the
provider's page in the provider's trust domain. This also means no provider
JavaScript inside our webview, which keeps the CSP strict (`script-src 'self'`).

---

## 3. THE DECISION THAT MATTERS: where the token lives

**A token in a file the agent can read is a token that lets the agent act outside
the gate.**

This is a new threat that does not appear in the current threat model because
nothing had a credential before. Writing it down as **T12**:

> **T12 — the credential the agent can reach.** The user's provider token is the
> authority to spend money and read data. If it sits in a plain file inside the
> state directory, the agent can read it, and then use it **directly** — with no
> gate, no approval, no card. The product's own guarantee becomes optional the
> moment its credential is readable.

That last sentence is the reason this section exists. It would be entirely
possible to build M5 with `token.json` in the state directory, watch it work
perfectly in every test, and ship a product whose central promise is void.

**Therefore:**

| | |
|---|---|
| **Storage** | **Windows Credential Manager**, via DPAPI (`CryptProtectData`) — never a plain file, not in the state directory, not in `AppData` |
| **Who holds it** | **The Rust shell**, not the sidecar. The sidecar is the component that runs alongside untrusted content; it should not be the component holding the spending authority |
| **Who uses it** | The shell makes the provider calls, or hands a token to the sidecar per-request with a short lifetime. **Not** "the sidecar keeps it forever" |
| **What is never logged** | The token, the code, the verifier. Not to the log file, not to the protocol stream, not to an error message |
| **Failure direction** | No token readable → **not connected.** Never "proceed unauthenticated and hope" |

**Honest limit:** DPAPI protects against another *user* on the machine. It does
**not** protect against a process running as the same user, which can call
`CryptUnprotectData` itself. So this raises the bar from "read a text file" to
"run code in the user's context that calls the same API" — meaningful, and **not**
the same as safe. The honest claim remains: the gate stops *the agent's proposed
actions*, and nothing here changes that this is a same-user boundary.

---

## 4. The spending cap

**The rule: a local cap is advice; a provider-side cap is a limit.**

- A cap the app enforces by counting is trivially bypassed by anything that can
  reach the provider directly — including the agent holding the token (T12).
- So the **default should be a provider-side limit**, set during onboarding, with
  the app's own counter as a *warning* rather than a control.

**Default posture: low, and visible.** A new user should see a monthly figure
before they can exceed it, and raising it should be a deliberate act with the
consequence stated plainly.

---

## 5. T6 must land with M5, not after

The threat model specifies it: **the approval must bind to a hash of the exact
action**, and the executor must verify that hash before running. M5 is when an
executor appears, so M5 is when T6 stops being a specification.

Cheap now, expensive later. The rule: *not "the same action" — the same bytes.*

---

## 6. What is built, and what is not

**Built and tested** (`src/onboarding/pkce.ts`, `tests/pkce.test.ts`):

- Verifier generation — 256 bits, RFC 7636 length bounds, base64url only
- S256 challenge, pinned against RFC 7636 appendix B so a weakened hash cannot pass
- `state` generation and **constant-time** comparison
- 25 tests, including the degenerate cases: empty state, truncated state, `+`/`/`/`=`
  characters leaking in from standard base64 (the classic silent failure — it
  works locally and the server rejects it, or computes a different challenge)

**Not built:** the loopback listener, the authorization URL, the token exchange,
credential storage, the cap UI, and the executor.

---

## 7. What is needed from Jeff

**An OpenRouter OAuth app registration.** Concretely:

1. A **client ID** (public — this one is safe to share, PKCE has no secret).
2. The **redirect URI** registered as a loopback address. Which port is a design
   question: a **fixed port** is simpler to register and more likely to conflict;
   an **ephemeral port** avoids conflicts but must be registered as a range, and
   not every provider supports that.

I will walk through the clicks when we get there, and I will not ask for a client
secret, because the flow does not have one.

---

## 8. Open questions

1. **Loopback port** — fixed or ephemeral (see §7).
2. **Where the executor lives.** T6 assumes something runs the approved action.
   Putting it in the shell means the sidecar never executes anything, which is a
   cleaner trust boundary and a larger refactor. **This is the decision that
   determines how much of M5 is a refactor rather than an addition.**
3. **Refresh.** Token lifetime and what happens at expiry mid-approval. The safe
   default — ask again rather than silently continuing — costs a click and is
   almost certainly right.
