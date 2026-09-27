# Bayesian Threshold Justification

> Why the defaults are where they are — written for the record, and for anyone
> who might want to tune them. Implements D14(2): the loss function and the
> priors behind every default, stated openly, with honest labelling of
> arbitrary choices.

---

## 1. The Loss Function

The classifier makes exactly one kind of error that matters to the product's
reason for existing, and one kind that costs attention. They are not
symmetrical, and the design treats them as commensurate only after weighing
them by many orders of magnitude.

**False negative (FN):** the agent executes a destructive action without human
approval. Outcome: data loss, credential exposure, unwanted payment, security
misconfiguration, an outbound message the human did not intend to send. These
are catastrophes — irreversible for the data, hard-to-detect for the
credentials, reputationally damaging for the message.

**False positive (FP):** the agent asks permission for an action that would have
been safe. Outcome: one click to approve, or one click to deny and a few
seconds of context-switching. Cost is bounded by attention and time, never by
data loss.

**Quantified asymmetry (estimate):** a single FN costs at least 10^4 times as
much as a single FP, by any measure that counts. A deleted project directory or
a credential push to a public endpoint costs hours to days of recovery. An
unnecessary prompt costs the user ~3 seconds of reading and a click. No
precision is claimed for the ratio; the order-of-magnitude gap is what
determines design decisions.

**Formal statement (for the record):** letting any loss-value function `L` be
defined over the set of outcomes, the design satisfies `L(FN) >> L(FP)` such
that no default may be tuned toward reducing FP at the cost of increasing FN.
Trading 10,000 unnecessary prompts for one missed destruction is a net loss.

This asymmetry is the root justification for every default below. It is also
the reason the document will call some thresholds "arbitrary but conservative":
when the cost of a wrong choice is this one-sided, any reasonable guess on the
safe side of the boundary is acceptable, and precision does not buy safety.

---

## 2. Thresholds

### 2.1 Per-tool baseline tiers

| Code reference              | Value     | Domain            |
|-----------------------------|-----------|-------------------|
| `BASE_TIERS` in tiers.ts    | see table | 21 known tool ids |

**Prior:** actions partition cleanly into four risk classes by their inherent
capacity for harm: reading nothing, creating something new (trivially
reversible), modifying or removing or sending (irreversible once executed),
and touching credentials, money, or the agent's own configuration (cannot be
un-done by any mechanism the product controls).

**Mapping rules (the prior expressed as a table):**

| Reading              | `silent`   | fs.read, fs.list, fs.stat, web.fetch, web.search, clock.read |
|----------------------|------------|--------------------------------------------------------------|
| Creating             | `notify`   | fs.create, fs.mkdir                                          |
| Modifying/removing   | `confirm`  | fs.write, fs.edit, fs.move, fs.rename, fs.delete, fs.truncate, db.delete, db.update, exec |
| Sending              | `confirm`  | message.send, mail.send, net.post, net.put, net.delete       |
| Secrets/money/config | `hard_gate`| credentials.read, credentials.write, spend, payment.send, config.security, agent.selfModify |

**Evidence that would move a given tool:** a concrete demonstration that the
action's inherent harm is systematically one class higher or lower than its
peers, measured against the loss function. For example, if every `fs.create`
call inside a build directory is undone by the next build, the `notify`
classification is appropriate; if a systematic study showed that "creating" a
critical config file by overwrite is functionally equivalent to `fs.write`, the
default would need to rise. No such evidence exists yet.

**Justification:** these baselines encode a structural prior about action
categories, not a per-tool empirical measurement. The categories are chosen to
match the loss asymmetry: destructive and outbound actions get human review;
secrets and money get the highest bar. The exact assignment of individual tools
to categories is a policy judgement, not a derivation, but it is the minimum
defensible categorisation.

**Load-bearing:** yes. Changing a baseline tier alters a safety guarantee that
holds for every call to that tool. Lowering any `confirm` baseline to `notify`
or `silent` would remove the human from the loop for a category of action that
the loss function says must have one.

---

### 2.2 Unknown tool fallback

| Code reference                | Value     |
|------------------------------ |-----------|
| `UNKNOWN_TOOL_TIER`           | `confirm` |

**Prior:** an unrecognised action is more likely to be dangerous than harmless.
The set of known tools is curated by the product authors and excludes anything
irrelevant or obviously safe. An unknown tool is therefore an action the
authors have either not yet evaluated, or have decided not to whitelist. Either
way, it defaults to the same tier as file deletion and command execution — the
confirm-requiring class.

**Evidence that would move it:** if the product gained a plugin system with
third-party tools whose safety profile is independently attested, the prior
could shift to `notify` for attested plugins. This is a product-architecture
change, not a tuning parameter.

**Justification:** the prior is the safe default by definition. "Fail safe" is
not Bayesian reasoning; it is the precautionary principle applied to a single
binary decision. The loss function justifies it overwhelmingly.

**Load-bearing:** yes. Every tool not in `BASE_TIERS` inherits this default.
Changing it makes all future unknown tools safer or less safe.

---

### 2.3 Taint-sensitive tools (untrusted motivation → hard gate)

| Code reference                | Value             |
|-------------------------------|-------------------|
| `TAINT_SENSITIVE_TOOLS` set   | exec, fs.delete, fs.truncate, db.delete, net.post, net.put, net.delete, message.send, mail.send |

**Prior:** tools that can destroy local data, destroy remote data, or send
communications have a qualitatively higher risk when motivated by untrusted
content. The prior is that an untrusted prompt to "delete everything in
Documents" is an adversarial attempt; an untrusted prompt to "fetch a web page"
is a legitimate instruction. The boundary between the two classes is the set
above — tools where a manipulated agent acting on untrusted content can cause
harm without any further user action.

**Evidence that would move a tool into or out of this set:** a specific
scenario where the tool's output is as benign as a read operation (moving it
out), or where a tool currently outside the set can cause destruction when
untrusted (moving it in). Net.post was included because it sends data to a
remote endpoint; net.get / web.fetch were excluded because they only receive.
This is a reasoned judgement, not a derivation.

**Justification:** the set is conservative by construction (it errs on the side
of including borderline tools). The loss function demands that any tool capable
of causing harm via untrusted motivation be hard-gated; the cost of a false
positive (hard-gating a web search that happens to be motivated by a web page)
is one click.

**Load-bearing:** yes. Removing a tool from this set removes a safety
guarantee around untrusted motivation for that tool. Adding tools is
conservative.

---

### 2.4 Blast-radius thresholds

| Code reference                | Value | What it does                          |
|-------------------------------|-------|---------------------------------------|
| `MASS_OPERATION`              | 10    | >10 items → raise(tier)              |
| `CATASTROPHIC_OPERATION`      | 100   | >100 items → hard_gate directly      |

**Prior:** the number of objects an action affects is an ordinal signal of how
much damage it can cause. The prior is that involving 11+ objects
simultaneously is a qualitatively different event from involving 10 or fewer,
and that involving more than 100 crosses into catastrophic territory regardless
of the action's baseline.

**Evidence that would move these thresholds:** user behaviour data showing the
modal range of "safe bulk operations" (e.g., renaming 8 files in a batch should
not be a hard gate). Telemetry on how often bulk operations at various sizes
are approved vs. denied. Actual incidents involving specific object counts.

**Justification:** the values 10 and 100 are arbitrary. They were chosen for
conservatism — the prior that "anything affecting more than ten things is
worthy of escalation" — but there is no principled derivation for 10 versus 8
or 12, nor for 100 versus 50 or 200. The tests (classify.test.ts) pin the
behaviour: 10 is the boundary that stays at `confirm`; 11 is the boundary that
escalates. The binary nature of the boundary is principled (at some count,
escalation must happen); the exact location is a guess.

**Load-bearing:** these thresholds are load-bearing in that setting them very
high (MASS_OPERATION = 1000, CATASTROPHIC_OPERATION = 10000) would allow bulk
destruction to pass at lower tiers. They are also the most obviously tunable
thresholds in the system, because the loss function (catastrophic FN vs 1-click
FP) provides a clear signal for adjustment: if real-world bulk operations at
count 15 are overwhelmingly safe, the threshold can rise without weakening any
guarantee at count 100.

---

### 2.5 Irreversibility escalation

| Code reference      | Rule                                                   |
|---------------------|--------------------------------------------------------|
| classify.ts:171-174 | `reversible === false` → `atLeast(tier, 'confirm')`    |
| classify.ts:175-176 | plus count > MASS_OPERATION → `hard_gate`              |

**Prior:** an action that cannot be undone is inherently more dangerous than
the same action when reversible. The minimum safe tier for any irreversible
action is `confirm` — the human must see it before it runs. When an
irreversible action also affects many objects (mass operation), the loss
function demands a hard gate, because a single mistaken approval destroys
everything at once.

**Evidence that would move it:** none that would lower it. The "at least
confirm" rule is structural: if an action cannot be undone, the human must be
in the loop. Telemetry showing that users approve irreversible mass operations
with high frequency might suggest the mass-operation threshold is too low, but
would not justify removing the confirmation requirement.

**Justification:** the `atLeast(tier, 'confirm')` rule is not a threshold in
the Bayesian sense; it is a safety invariant. An irreversible action that runs
silently or with only a notification is a false-negative incident waiting to
happen.

**Load-bearing:** yes. Removing this rule would let irreversible actions run
without human review at the `notify` or `silent` tier, directly violating the
loss function.

---

### 2.6 Leaving-machine escalation

| Code reference      | Rule                                                    |
|---------------------|---------------------------------------------------------|
| classify.ts:178-180 | `leavesMachine === true` → `atLeast(tier, 'confirm')`   |

**Prior:** actions that reach outside the machine (network calls, emails,
messages) expose the user to external risk regardless of the action's local
safety profile. The prior is that exfiltration, unwanted publication, and wire
fraud are FN events the product must prevent.

**Evidence that would move it:** the rule is structural. The "at least confirm"
boundary is the minimum to satisfy the loss function. There is no evidence
path to lowering it below `confirm`.

**Justification:** same category as irreversibility — a safety invariant, not a
tunable parameter.

**Load-bearing:** yes.

---

### 2.7 Untrusted taint escalation

| Code reference      | Rule                                                                  |
|---------------------|-----------------------------------------------------------------------|
| classify.ts:184-189 | taint untrusted/mixed → `atLeast(tier, 'confirm')`; and if tool in TAINT_SENSITIVE_TOOLS → `hard_gate` |

**Prior:** a motivation from untrusted content is presumptively adversarial.
The prior is that an agent acting on what it read in a page, file, or message
is more likely to be manipulated than an agent acting on a direct human
instruction.

**Evidence that would move it:** see section 2.3 (the set of tools) and the
fact that the "at least confirm" rule mirrors irreversibility and leaving: it
is structural. The distinction between untrusted and trusted motivation is
binary; the prior says untrusted → escalate. That is conservative by design.

**Justification:** this rule exists because the Replit incident (D6, D10)
involved an agent acting on untrusted content during a code freeze. The
product's defining test is that Capybaras halts at this gate. The prior is
deliberately strong.

**Load-bearing:** yes. Lowering this would replicate the Replit failure mode.

---

### 2.8 Hard disqualifiers (structural)

| Field               | Escalation |
|---------------------|------------|
| `protectedTarget`   | `hard_gate` |
| `touchesSecrets`    | `hard_gate` |
| `involvesMoney`     | `hard_gate` |
| `changesSecurityConfig` | `hard_gate` |

**Prior:** some action properties are inherently disqualifying. The action
cannot proceed at any lower tier regardless of its tool, blast radius, or
reversibility. The prior is that touching a protected target, secrets, money,
or security settings carries an unconditional risk of catastrophic FN.

**Evidence that would move them:** none — these are structural invariants. The
labels are binary flags on the `ActionDescriptor`. There is no tuning path.

**Justification:** these are not thresholds with a continuous value; they are
categorical overrides. The loss function says that an FN with these properties
is always catastrophic, so the interruption must always be maximal.

**Load-bearing:** yes. Removing any one of these removes the guarantee that the
corresponding hazard always stops for human input.

---

### 2.9 Destructive pattern detection

| Code reference            | 12 regex patterns (classify.ts)                         |
|---------------------------|----------------------------------------------------------|
| Effect                   | Matching any → `hard_gate` with a named reason           |

**Prior:** a command argument containing `rm -rf`, `DROP TABLE`, `mkfs.ext4`,
or any of the listed patterns is presumptively destructive regardless of the
tool's baseline. The prior is that these patterns are almost never typed by
accident for a legitimate purpose.

**Evidence that would move a pattern out:** a corpus showing that a given
pattern occurs in legitimate shell commands more often than in destructive
ones. For example, `git clean -fd` appears in legitimate CI scripts; the
product author judged it destructive enough to keep. The pattern set is a
heuristic, not a derivation.

**Evidence that would add a pattern:** discovery of a new pattern that is
commonly used to destroy data (e.g., `repair-volume /f /r` on a mounted disk).

**Justification:** the pattern list is a heuristic with arbitrary membership.
It errs toward false positives (flagging `git clean -fd` as destructive when
the user intended it) because the loss function says an unnecessary prompt
costs one click and a missed destruction costs data. The specific 12 patterns
are chosen from common security-incident knowledge; no principled derivation
proves that 12 is the right number or that these are the right 12.

**Load-bearing:** partially. Adding patterns is conservative and costs nothing
but patterns to verify. Removing a pattern weakens a safety net but the net is
heuristic rather than a guarantee. The most load-bearing aspect is the
escalation to `hard_gate` on any match.

---

### 2.10 allowRemember conditions

| Code reference | Conditions for `allowRemember = true`                                   |
|----------------|-------------------------------------------------------------------------|
| classify.ts    | tier === `confirm` && taint === `trusted` && reversible !== false && protectedTarget !== true |

**Prior:** a remembered grant is a privilege, not a right. It applies only to
`confirm`-tier actions that the human explicitly requested (trusted), that can
be undone (reversible), and that do not involve off-limits targets.

**Evidence that would move it:** none. These are structural safety properties.
A hard-gated action must always ask; an untrusted action must always ask; an
irreversible action must always ask. Allowing any of those to be remembered
would let a manipulated agent or a destructive action bypass human review
through accumulated approvals.

**Justification:** these are invariants, not thresholds. The conditions are
individually necessary and jointly sufficient. There is no tuning dimension.

**Load-bearing:** yes. The entire grant system depends on these conditions.

---

### 2.11 Grant default time-to-live

| Code reference                | Value              |
|-------------------------------|--------------------|
| `DEFAULT_TTL_MS` in grants.ts | 30 × 24 × 60 × 60 × 1000 = 2,592,000,000 ms (30 days) |

**Prior:** a remembered permission should expire before the user forgets they
granted it, but not so soon that it becomes effectively useless for routine
workflows. The prior is 30 days: long enough to cover a typical project phase,
short enough that most users will revisit the decision at least monthly.

**Evidence that would move it:** usage data showing that a 30-day TTL causes
excessive re-prompting for legitimate daily workflows (suggesting a longer
default, say 90 days), or that approvals of dangerous actions tend to
concentrate just before expiry (suggesting a shorter default, say 7 days). The
Tunable parameter has a clear empirical signal: if the ratio of "re-approve a
remembered grant" to "first-time approve" is high, the TTL is too short; if
users rarely re-encounter a grant before it expires, the TTL is adequate.

**Justification:** 30 days is arbitrary. It has no Bayesian derivation. It is a
conventional choice based on: it is divisible into calendar months; it matches
common SaaS trial and billing cycles; it is long enough to be useful. No data
informs it yet. This is the threshold most suitable for tuning once the product
has real usage, because the loss function is symmetric: setting it too short
increases FP (unnecessary prompts), setting it too long increases FN risk
(stale permissions on a computer no longer attended in the same way).

**Load-bearing:** not in the safety sense. No safety guarantee depends on a
30-day default. The safety guarantee is that a grant *expires*, enforced by
`DEFAULT_TTL_MS` being a finite positive number. The exact value is
conventional.

---

### 2.12 Grant scope constraints (wildcard rejection)

| Code reference | Rule                                                                  |
|---------------|-----------------------------------------------------------------------|
| grants.ts:74  | `WILDCARD = /[*?]|^all$|^any$|^everything$/i` — matched scope → refusal |

**Prior:** a permission scoped to a pattern ("all files in Documents") or a
wildcard ("*") is the failure mode this product exists to prevent. The prior is
that imprecise grants cause FN incidents. The Replit incident (an agent told
"do not touch production" but with delete access to everything) is the
existence proof.

**Evidence that would move it:** none. This is a structural invariant grounded
in a specific incident. There is no scenario in which a wildcard grant is
safe given the loss function, because a manipulated agent could use a wildcard
grant to cause destruction without further human intervention.

**Justification:** the product's existence is premised on the premise that
broad, unexpiring permissions are dangerous. Relaxing this rule would
contradict the product's reason for existing.

**Load-bearing:** yes. Changing this rule removes the single strongest
structural protection against *post-approval* catastrophic action.

---

### 2.13 Grant provenance (only human may grant)

| Code reference | Rule                                                                    |
|---------------|-------------------------------------------------------------------------|
| grants.ts:84  | `record()` demands `approvedBy: 'human'` and refuses anything else      |

**Prior:** the model has no legitimate path to self-authorise. Any approval
must be traceable to a human decision. The prior is that a model that can
create its own grants is no longer safety-gated.

**Evidence that would move it:** none. This is a structural invariant. "The
model may create grants" is not a tunable parameter; changing it removes the
foundation of the entire approval system.

**Justification:** see D14 — "the model has no code path that creates a grant."
This is enforced by the type signature.

**Load-bearing:** yes. Core invariant.

---

## 3. Summary: Load-Bearing vs. Conventional

| Threshold                      | Load-bearing | Notes                                                  |
|-------------------------------|--------------|--------------------------------------------------------|
| Per-tool baseline tiers       | yes          | Category assignment is judgement; changing any lowers a guarantee |
| Unknown tool fallback         | yes          | Structural: fail-safe                                  |
| Taint-sensitive tool set      | yes          | Membership is reasoned, not derived                    |
| Mass operation (10)           | partially    | The concept (escalate at some count) is load-bearing; the exact number 10 is arbitrary |
| Catastrophic operation (100)  | partially    | Same as mass operation                                 |
| Irreversibility → confirm     | yes          | Structural invariant                                   |
| Leaves machine → confirm      | yes          | Structural invariant                                   |
| Untrusted taint → confirm     | yes          | Structural; grounded in the Replit incident            |
| Hard disqualifiers            | yes          | Structural overrides                                   |
| Destructive patterns          | partially    | Pattern membership is heuristic; the *concept* of matching is load-bearing |
| allowRemember conditions      | yes          | Structural safety invariants                           |
| Grant TTL (30 days)           | conventional | Safe to tune; no guarantee depends on the exact number |
| Grant scope (no wildcards)    | yes          | Structural; grounded in the Replit incident            |
| Grant provenance (human only) | yes          | Structural; core invariant                             |

## 4. What Data Would Justify Tuning

**Legitimate tuning signals (each answers a specific question):**

- **Blast-radius thresholds:** distribution of `affectedCount` across all
  classified actions, cross-referenced with human approval/denial outcomes. If
  operations with affectedCount=15 are denied 0.1% of the time (humans
  consistently approve them), the mass-operation threshold could be raised
  without adding FN risk. The data must show that the threshold triggers on
  safe operations, not that safe operations exist at higher counts.
- **Grant TTL:** histogram of grant-lifespan-to-reapproval intervals. If most
  grants are re-approved at 29 days, the TTL is too short for the user's
  workflow. If grants expire and are never missed, the TTL is long enough.
- **Destructive-pattern false-positive rate:** of all `exec` calls classified
  as matching a destructive pattern, what fraction did the human approve? A
  high approval rate (e.g., 90%+ of `rm -rf /var/tmp` calls are approved)
  suggests the pattern is too broad. But the approval rate does not mean the
  pattern is safe — only that the human, having seen the context, decided it was
  safe. The feature is working correctly. Remove a pattern only if there is
  evidence it never matches actual destructive intent.
- **Untrusted-action denial rate:** what fraction of untrusted confirmations or
  hard-gate prompts are denied? A very high denial rate validates the
  untrusted=escalation rule. A very low rate could suggest too many false
  positives, except that the denominator includes automated or adversarial
  inputs — the data are only interpretable with ground truth about intent.

**Illegitimate tuning signals:**

- **"Too many prompts, reduce them."** This is an optimisation against the loss
  function. Reducing prompts by lowering thresholds increases FN risk. The
  correct response to "too many prompts" is: improve the grant system (better
  TTL, smarter defaults, per-target grants), audit which specific prompts are
  noise, and fix upstream disclosures that produce false positives. It is never
  to lower a threshold directly.
- **"The model works better when we approve everything."** Model performance on
  downstream tasks is not a safety signal. The safety gate does not optimise
  for model throughput.
- **"We never had an incident."** The absence of evidence is not evidence of
  absence. An FN has not happened yet; that does not mean a threshold was too
  high.

---

## Appendix A: Thresholds Not Derived (Honest Inventory)

The following have no principled justification. They are guesses, chosen for
conservatism, set to round numbers, and documented here so that future
engineers do not spend time deriving what cannot be derived:

- **MASS_OPERATION = 10** and **CATASTROPHIC_OPERATION = 100** — no derivation
  exists. The binary boundary (escalate at N+1) is necessary; N is arbitrary.
- **DEFAULT_TTL_MS = 30 days** — no derivation. A conventional duration.
- **Pattern inclusion for `destructive git clean` (`git clean -fd`)** — the
  pattern is a legitimate CI/CD command. Its inclusion is conservative by
  intent; a principled justification would require a corpus of `git clean` uses
  and incident reports. No such corpus exists.
- **Membership of `TAINT_SENSITIVE_TOOLS`** — the set is reasoned policy, not
  derived from data. The inclusion of `net.put` (put, not post) and exclusion
  of `net.patch` is a judgement call.

These are not defects. They are honest choices in a system whose loss function
does not reward precision at the boundary. The conservative side of an
arbitrary boundary is the correct side.

---

## Appendix B: What D14 Explicitly Excludes

D14 settled two things relevant to this document:

1. **No probabilities in the tier decision.** Every threshold above is a
   discrete boundary (a count, a boolean, a set membership). The classifier
   does not compute posteriors. The product's guarantees (escalation-only,
   unknown→confirm, hard gates never remembered) are structural properties of
   a rule system and are provable by reading the rules — not statistical
   properties that require interpretation.

2. **A soft-signal scorer is deferred.** The one place Bayesian machinery could
   legitimately appear — a probabilistic soft-signal scorer that can only raise
   tiers — is considered and deferred until the deterministic core is proven
   end-to-end (M4, the Replit acceptance gate in a real UI). This document does
   not describe that scorer, because it does not exist yet.