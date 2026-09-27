# The shell ↔ sidecar protocol, version 1

**Status:** M4.1. This is an interface, not an implementation detail — which means it gets versioned, documented, and tested, because the expensive failure is designing it twice.

---

## Why this exists

The shell is Rust. The policy layer is TypeScript. **Nothing connected them.** Before M4 the arrangement was:

- the shell sent, the sidecar obeyed — one direction only;
- the shell discarded the sidecar's stdout entirely (`.stdout(Stdio::null())`), so the sidecar *could not* reply;
- exactly one message existed: `{"cmd":"shutdown"}`.

An approval interface cannot be built on that. It needs a conversation.

---

## Framing

**Newline-delimited JSON (NDJSON).** One complete JSON object per line, UTF-8, terminated by `\n`. No length prefix, no nesting of frames.

Chosen because both ends already parse lines, it is trivially debuggable by eye, and a malformed line costs one message rather than the stream.

Rules:
- **A line that is not valid JSON is ignored and reported** — never fatal. A bad message must not take down the agent.
- **Unknown `type` values are ignored** — forward compatibility in one direction; the `v` field handles the other.
- **Every message carries `v`.** A peer receiving an unsupported `v` refuses the message and says so, rather than guessing.

---

## Envelope

```jsonc
{ "v": 1, "type": "<type>", "id": "<optional correlation id>", ...payload }
```

`id` is present on anything that expects a reply, and echoed back so replies correlate. It is a string.

---

## Shell → sidecar

| Type | Payload | Meaning |
|---|---|---|
| `action.propose` | `id`, `action`, `context?` | The agent wants to do something. This is the core message. |
| `approval.answer` | `id`, `decision` | A human answered. `decision` is `allow`, `deny`, or `remember`. |
| `grants.list` | `id` | Ask what has been remembered. |
| `grants.revoke` | `id`, `scope` | Forget one remembered grant. |
| `shutdown` | — | Stop. The graceful path. |

`action` is an `ActionDescriptor` as the classifier defines it: `{ tool, args?, affectedCount?, reversible?, touchesSecrets?, involvesMoney?, changesSecurityConfig?, ... }`.

`context` is `{ targetLabel?, dryRun? }` — display and policy hints, never authority.

---

## Sidecar → shell

| Type | Payload | Meaning |
|---|---|---|
| `ready` | `pid`, `node`, `protocol` | Said once, at startup. |
| `action.proceeded` | `id`, `tier`, `because` | Cleared to run. |
| `action.dry_run` | `id`, `headline`, `reasons` | Shown, not run. |
| `approval.required` | `id`, `request` | **Blocked, and waiting for a human.** |
| `approval.resolved` | `id`, `outcome` | What the human decided. |
| `grants.listed` | `id`, `grants` | Reply to `grants.list`. |
| `error` | `id?`, `message` | Something was refused or failed. |
| `log` | `level`, `message` | Diagnostics. |
| `heartbeat` | `uptimeMs` | Liveness. |

`request` is an `ApprovalRequest`: `{ tier, headline, reasons[], confirmationPhrase?, canRemember, requiresTypedConfirmation, target }`.

---

## The flow for an action that needs a human

```
shell  →  action.propose      { id: "a1", action: {...} }
sidecar   ...runs the gate...
sidecar →  approval.required  { id: "a1", request: {...} }
            <<< the sidecar now blocks. nothing proceeds. >>>
shell  →  approval.answer     { id: "a1", decision: "allow" }
sidecar →  approval.resolved  { id: "a1", outcome: "allowed" }
sidecar →  action.proceeded   { id: "a1", tier: "confirm", because: "..." }
```

---

## The rule this protocol exists to make true

> **An action at `confirm` or `hard_gate` cannot proceed without an `approval.answer`. There is no other path.**

Concretely, and each of these is a test:

1. **The sidecar blocks.** A proposed action that needs a human stays pending indefinitely. It does not time out into approval, and it does not proceed optimistically.
2. **`deny` is final for that action.** It never becomes a `proceed`.
3. **A hard gate is never satisfied by memory.** `decision: "remember"` cannot record a grant for a hard gate — the classifier already refuses to offer it (`allowRemember` is false), and the sidecar refuses independently even if asked.
4. **An unsolicited `approval.answer` is refused.** An answer for an action that was never proposed, or already resolved, is an error — never a queued approval waiting to be spent on the next action.
5. **The shell has no path that auto-approves.** No default, no fallback, no "trusted target" shortcut.

Point 4 is the subtle one. A naive implementation that resolves "the next pending approval" would let a stray or replayed message approve something else entirely.

---

## Compatibility note

The version-1 handshake keeps the pre-M4 shutdown contract working: `{"cmd":"shutdown"}` remains accepted as well as `{"type":"shutdown"}`. Today's supervision tests assert that a graceful stop reaches the sidecar's own handler, and that guarantee must survive this change rather than be quietly re-broken.
