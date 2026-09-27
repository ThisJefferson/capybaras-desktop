# Changelog

All notable changes to Capybaras. Format loosely follows [Keep a Changelog](https://keepachangelog.com/); versioning is semver, and **nothing is stable before 1.0**.

Test counts and notable findings are included deliberately — the acceptance suite is the thing that tells us when a safety guarantee has been broken, so its result belongs in every entry.

---

## [Unreleased]

### In progress
- **Policy layer** — binding the classifier to a grant store (scoped, expiring, human-only) and a dry-run mode. This is what turns a classification into an actual gate.

### Planned
- Tauri shell + Node sidecar (spike first)
- Approval interface, the herd, receipts
- OpenRouter OAuth onboarding

---

## [0.0.2] — 2026-09-27

### Added
- **Policy layer** — `src/policy/`. The gate turns a classification into an actual decision: `proceed`, `ask`, or `dry_run`.
  - `GrantStore` remembers **one action against one specific target**, expiring after 30 days by default.
  - Approval requests carry **every** reason the classifier found — not just the first — in plain language with no tool ids.
- **Community documents** — `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.github/FUNDING.yml`, and issue templates.
- **Brand assets** — six SVGs in `assets/brand/`: four capybara states (sleep, listening, working, needs-you), a six-capybara herd strip, and a 512 px app icon.
- **Packaging and signing research** — `docs/research/packaging-and-signing.md` (31 KB), with corrections recorded as `DECISIONS.md` D11.
- **CI on every push** — Windows runner, Node 24. Runs typecheck and the full suite, and publishes a test-results summary to the run page.

### Rules enforced, each with tests
- A remembered grant satisfies a `confirm` **and nothing else**. **A hard gate is never satisfied by memory**, however many times it was approved.
- **Dry run can only add caution.** It never turns an `ask` into a `proceed`.
- Grants are scoped to one action against one target — **wildcards are structurally impossible**, because the store refuses to hold one.
- **The model has no code path that creates a grant.** `record()` demands human provenance *and* a classification that already permits remembering.

### Test results
**141 tests passing · typecheck clean** (was 99)

| Suite | Cases |
|---|---|
| `tests/classify.test.ts` | 84 |
| `tests/policy.test.ts` | 42 |
| `tests/replit-acceptance.test.ts` | 15 |

### Corrections to previously stated facts
- **The Microsoft Store developer account is free.** The fee was removed for individual developers in September 2025 and for companies on 7 May 2026. Previously budgeted at $19 in this repository.
- **Azure Artifact Signing does not grant instant SmartScreen reputation**, contrary to third-party marketing. Microsoft's own documentation says reputation is built over time.

### Changed
- `BUILD-PLAN.md` distribution section corrected for both of the above, plus a new architecture note: **MSIX has no Scheduled Tasks**, so the Gateway's auto-start must become an MSIX Start-up Task under Store distribution.

### Known unverified risk
Whether a bundled **Node sidecar** behaves correctly inside an MSIX *full-trust* package. No single source documents MSIX + Tauri sidecar + Store submission together, so the Phase 2 spike must test that specific case before packaging work begins.

---

## [0.0.1] — 2026-09-27

### Added
- **Risk classifier** — `src/risk-classifier/`. Classifies every proposed action into one of four tiers *before it runs*:
  - **silent** — reads, searches
  - **notify** — creates something
  - **confirm** — deletes, edits, moves, sends, runs a command
  - **hard_gate** — mass operations, irreversible change, money, secrets, security config, protected targets
- Hazard rules covering blast radius, reversibility, egress, motivation taint, destructive command patterns, protected targets, secrets, money, and security-config changes.
- **The acceptance suite** — `tests/replit-acceptance.test.ts`, encoding the incident of 23 July 2025.

### Invariants
- **Escalation only.** Rules may raise a tier. Nothing lowers it.
- **Classification is the system's, not the model's.** A manipulated agent cannot argue its way down a tier.
- **Unknown fails safe.** An unrecognised tool requires consent rather than being treated as harmless.

### Test results
- **99 tests passing · typecheck clean**
  - `tests/classify.test.ts` — 84 cases
  - `tests/replit-acceptance.test.ts` — 15 cases

### Notable finding
The acceptance test caught a real defect on its first run. The classifier originally recorded a reason **only when the tier moved**, so an action that was simultaneously off-limits, 1,200 rows, and irreversible reported exactly **one** reason: *"off-limits."*

That is the precise failure this project exists to prevent — a human approving something dangerous while being shown only the first thing that made it dangerous. Tier and reasons are now computed independently (see `DECISIONS.md` D10).

### Repository
- Scaffold, project documents, Apache-2.0 licence, OpenClaw's MIT notice vendored to `LICENSES/`, third-party notices, and the Replit acceptance test as the defining criterion.
