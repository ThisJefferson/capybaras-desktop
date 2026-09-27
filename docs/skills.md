# Skills

**A skill is a capability. A capability is an attack surface.** Adding skills is therefore a safety-relevant act, not a feature toggle — and this document is about doing it without quietly dismantling the gate.

---

## 1. The tension, stated first

Capybaras works because it **knows what the agent can do**. Every tool is classified before it runs, and an unrecognised tool falls safe to `confirm`.

That property is what makes a skill suite dangerous as well as useful:

- Every skill added is a new entry that must be classified **correctly**.
- A mis-classified skill is not a small bug. It is precisely the hole the product exists to prevent.
- The suite grows over time, usually by people adding capability rather than danger.

So the rule this document exists to enforce: **a skill cannot be added without declaring how dangerous it is.** There is no default. Silence is a build error, not a lenient classification.

---

## 2. How a skill declares itself

Today the tool knowledge is scattered: a `VERBS` table in the gate for plain-language headlines, a `TOOL_OWNER` table in the sidecar for the herd, and the tier logic in the classifier. Three lists that can disagree.

The skill registry should be **one declarative manifest**, and classification should read it:

```jsonc
{
  "tool": "pdf.merge",
  "headline": { "one": "Combine PDFs into one", "many": "Combine {n} PDFs into one" },
  "owner": "nina",                       // which capybara
  "readsUntrusted": true,                // input may be adversary-authored
  "leavesMachine": false,
  "reversible": true,
  "blastRadiusSensitive": true,          // affectedCount escalates the tier
  "tier": "notify"                       // the floor; escalation can only raise it
}
```

**Three rules make the manifest the boundary:**

1. **Missing field → the build fails.** Not "assume safe". The token bug in this project was exactly this shape — a lookup that returned nothing and was silently skipped, producing a UI that looked almost right. The same failure in a skill table would produce a tool that is almost safe.
2. **The declared tier is a FLOOR, never a ceiling.** Escalation on reversibility, blast radius, taint and protected targets still applies. A manifest cannot make something quieter than the classifier would.
3. **Unknown tool → `confirm`.** Already true, and it stays true: the fallback must never be permissive, or a manifest that fails to load becomes an open door.

---

## 3. The PDF family, as the first suite

PDF is a good first suite precisely because it is *not* innocuous. A PDF is a document someone else wrote, and it can carry a script, an embedded file, or a launch action. Reading one is reading untrusted input.

Suggested classification:

| Tool | Tier | Why |
|---|---|---|
| `pdf.read`, `pdf.extract-text` | **silent** | Perception. It reads. Nothing changes |
| `pdf.info`, `pdf.page-count` | **silent** | Metadata only |
| `pdf.create`, `pdf.merge`, `pdf.split` | **notify** | Creates something new; the originals are untouched |
| `pdf.convert` | **notify** | Same, though it may shell out to a converter |
| `pdf.overwrite`, `pdf.delete-pages`, `pdf.rotate` | **confirm** | Mutates a file that already existed |
| `pdf.fill-form`, `pdf.sign` | **confirm** | Signing is a legal act; a wrong signature is not merely inconvenient |
| `pdf.redact` | **confirm**, and see below | |
| `pdf.decrypt` | **hard gate** | It may require a password, and it may be someone else's document |
| `pdf.attach-file` | **hard gate** | It writes arbitrary bytes into a document that may then be sent |

### Why redaction deserves its own paragraph

Redaction is the one skill here where **the failure mode is invisible and permanent.** A redaction applied as a black rectangle over text leaves the text intact underneath, extractable by anyone. The harm does not announce itself — the document *looks* redacted.

So a redaction skill needs two properties beyond a tier:

- It must **verify** the content is gone, not merely covered, and say so.
- It must **surface the verification** in the approval card and in the receipt. "Redacted 4 passages" is not enough; "removed the underlying text, verified by re-extraction" is.

This is the general lesson for skills: some capabilities need **post-conditions**, not just tiers.

---

## 4. The combination rule

The genuinely dangerous cases are not individual skills. They are **combinations**, and the classifier reasons about one action at a time.

The combination that matters is the familiar one: **read untrusted content + hold file access + send externally.** Any two are survivable. All three together are an exfiltration path.

Two consequences for the skills design:

1. **`readsUntrusted` must be declared and must taint the process.** If a PDF's contents are attacker-authored and the agent then proposes to send something, that send is not a clean action.
2. **A skill that reads untrusted content is never `silent` when it also leaves the machine.** A `pdf.extract-text` followed by a `message.send` should be treated as one story, not two unrelated events.

How far to take this — taint tracking across a session, or a simpler "was untrusted content read recently" flag — is **not yet decided.** It is flagged here rather than guessed at, because guessing would put a weak mechanism in a load-bearing place.

---

## 5. What we deliberately will not do

**We will not implement PDF manipulation in the safety core.** The gate's job is to classify and interrupt, not to parse PDFs. It should call well-maintained libraries, or let the agent use its own tooling.

The reason is the same one behind the whole design: **every line of parsing code added to the trusted path is a line of attack surface inside the thing that is supposed to be trustworthy.** A PDF parser is a historically reliable source of memory-safety bugs. It does not belong next to the decision logic.

---

## 6. Order of work

1. **The manifest and its validation.** One declarative source of truth, with a missing field failing the build. This is the prerequisite for everything else — without it, every added skill is a new opportunity for silent leniency.
2. **A `docs/skills/` directory of manifests**, so the surface is readable and reviewable as data.
3. **The PDF family**, classified per §3, with the redaction post-condition.
4. **Taint across a session** (§4), once designed properly rather than sketched.
5. **A skills review checklist**, so that adding one is not only permitted but *deliberate*: what does it read, what does it change, what can it reach, what happens when it is wrong?

---

## 7. The one-line rule

> **Every skill must declare how it can hurt you, and a skill that forgets to will not build.**
