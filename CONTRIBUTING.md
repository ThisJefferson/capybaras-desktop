# Contributing to Capybaras

**Thank you for being here.** This project is early, and every thoughtful contribution matters.

Capybaras is a safety product. The bar for contributions is higher than usual, and that is intentional. The thing that makes a vulnerability in a code editor annoying is the same thing that makes a vulnerability in an agent dangerous: it can act. Every change is judged against that responsibility.

---

## Prerequisites

- **Node 24** (match the `engines` field in `package.json`)
- **npm** (ships with Node)
- **Git** (any modern version)
- **Rust toolchain** — only needed for Tauri-sidecar work (see `BUILD-PLAN.md`)

The project uses TypeScript, tested with Vitest. No global installs needed beyond Node.

---

## Getting started

```bash
npm install
```

That installs everything. No separate steps for the Rust side unless you are working on the Tauri shell.

---

## Running checks

```bash
npm test            # full Vitest suite
npm run typecheck   # TypeScript type checking (tsc --noEmit)
```

**CI runs both on every push.** Keep both clean before opening a pull request.

There is also a focused acceptance test:

```bash
npm run replit-test  # only the Replit acceptance test
```

This is the one test the whole product is judged against. If it fails, nothing else matters.

---

## How to add or change a classifier case

The risk classifier lives in `src/risk-classifier/`. It is the core of the product.

**The rule is simple: change the test and the implementation together, in the same commit.**

1. Add or update a test case in `tests/classify.test.ts` that describes the new behaviour.
2. Change the implementation in `src/risk-classifier/` to make it pass.
3. Record the reasoning in `DECISIONS.md` — what the case covers, why the tier is what it is, what was considered and rejected.
4. Run the full suite (`npm test`) to confirm nothing else broke.

**The acceptance suite must never be weakened to make a change pass.** If a change breaks the acceptance test, the change is wrong — unless you can argue otherwise in `DECISIONS.md` and persuade a reviewer. The acceptance test is not a rubber stamp. It is the product's spine.

---

## Commit style

Use [conventional commits](https://www.conventionalcommits.org/):

```
feat: add shell-execution tier for command-line tools
fix: replit test catches missing "cannot be undone" reason
docs: explain the blast-radius heuristic in DECISIONS.md
ci: enable workflow scope for auto-tests
chore: bump vitest to 3.2.7
```

Imperative mood, sentence case, no full stop at the end of the subject line.

Scope is optional but helpful for the classifier: `classifier:`, `policy:`, `ui:`.

---

## Before opening a pull request

- [ ] `npm run typecheck` passes clean
- [ ] `npm test` passes (all tests, not just yours)
- [ ] `npm run replit-test` passes
- [ ] If you touched the classifier, `DECISIONS.md` records the reasoning
- [ ] Commits follow conventional commit format

---

## What to expect

This is a volunteer project. Reviews may take a few days. There is no entitlement to speed.

If your change weakens the acceptance test, the reviewer will block it. That is not personal. The Replit incident is the reason this project exists, and the test that proves it works cannot be compromised.

---

## Code of conduct

By participating, you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md). Be calm. Be curious. Assume good faith.