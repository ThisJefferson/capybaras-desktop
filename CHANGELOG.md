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
- **CI on every push** — Windows runner, Node 24. Runs typecheck and the full suite, and publishes a test-results summary to the run page so the acceptance outcome is visible without opening logs.

### Fixed
- Nothing. (The workflow itself previously could not be pushed: the GitHub token lacked the `workflow` scope. Worked around by holding the file out of commits; the scope has since been granted and the file is now tracked.)

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
