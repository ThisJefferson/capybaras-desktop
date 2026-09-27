# Capybaras

**A calm herd of helpers that ask before they act.**

[![ci](https://github.com/ThisJefferson/capybaras-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/ThisJefferson/capybaras-desktop/actions/workflows/ci.yml) ![pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange) ![licence](https://img.shields.io/badge/licence-Apache--2.0-blue)

> Desktop agent that stops and checks with you before doing anything that matters.

**Status: early development.** Nothing is installable yet. This repo is being built in the open.

---

## Why this exists

This project starts from a specific, evidence-backed fear, and it is worth stating plainly rather than dressing it up.

Research compiled for this project puts the probability of **serious harm to society from agentic AI within ten years at roughly 70%.** The companion estimate — that AI-orchestrated offensive operations become routine in the same window — is **~90%.**

The evidence is in this repo, not asserted from nowhere: [`docs/research/agentic-harm.md`](./docs/research/agentic-harm.md) and [`docs/research/safe-agent-blueprint.md`](./docs/research/safe-agent-blueprint.md).

**Neither figure is a prophecy, and neither depends on a rogue superintelligence.** The harms are ordinary, and most of them are already documented:

- **Destructive autonomous action needs no attacker at all.** A coding agent deleted a production database during a declared code freeze, then misreported it. A CLI agent deleted a user's files. Another deleted production data. Each was a wrong assumption plus over-broad permissions plus no gate between deciding and doing.
- **Prompt injection may be a permanent property of the architecture, not a defect that gets fixed** — the conclusion of a June 2026 analysis, not a pessimistic guess. Zero-click injection has already been demonstrated against production systems (EchoLeak, CVE-2025-32711), and malicious *tool descriptions* have been shown to function as obeyed instructions.
- **AI-enabled offensive cyber is the genuinely new capability.** In November 2025 Anthropic published the first reported AI-orchestrated espionage campaign, in which Claude Code instances executed **80–90% of tactical operations autonomously**, at request rates no human team could sustain, across roughly 30 organisations.

The window is measured in years, and the failures are mundane. That is the point.

**Capybaras is an attempt to make that failure small, visible and reversible.**

---

## Who this is for, and the deal it makes

Two commitments that are easy to state separately and hard to hold together:

**It has to work for people who are not technical.** No API keys pasted into config files. No terminal. No vocabulary to learn before you can start. If using this safely requires understanding it, then safety stays a privilege for people who already had it.

**Safety cannot mean useless.** A tool that interrupts constantly gets switched off, and a switched-off guard rail protects nobody. So the bar is not *"how often does it stop you?"* — it is **"does it stop you only when it matters, and is it genuinely worth using the rest of the time?"** An agent that is safe and useless has failed as surely as one that is useful and dangerous.

**And the honest limit:** this cannot make an agent safe, because no agent that can act on your behalf is safe. What it can do is make failure small, visible, and reversible — and put a human in the loop at exactly the moments that matter.

---

## The problem

AI agents that can actually *do* things — edit your files, send your messages, run commands — are genuinely useful and genuinely dangerous. The failure mode is not science fiction. It is boring and it already happened:

> **23 July 2025.** An AI coding agent deleted a production database *during an explicitly declared code freeze*. It wiped data for more than **1,200 executives and 1,190 companies**, then misreported what it had done.

No attacker. No malicious goal. A wrong assumption, over-broad permissions, and no gate between deciding and doing.

## What Capybaras does about it

**Every consequential action stops and asks.**

Actions are classified into risk tiers before they run:

| Tier | Behaviour |
|---|---|
| **Silent** | Reads, searches — logged, not interrupted |
| **Notify** | Creates something — runs, then tells you |
| **Confirm** | Deletes, edits, sends — **pauses and asks** |
| **Hard gate** | Mass deletion, money, credentials — typed confirmation + reason |

Classification is done by the **system, not the model** — an agent that has been manipulated cannot argue its way to a lower tier.

And confirmation always arrives **out-of-band from the untrusted input.** If the agent read a web page that told it to act, the approval prompt cannot be influenced by that page.

The prompt itself follows one rule above all others:

> **Cancel is the default. Enter never approves something destructive.**

## The herd

You don't talk to a faceless process. A small herd of capybaras works for you, and you can *see* what each one is doing — which matters, because a multi-agent system is otherwise completely invisible.

- **Tuca** — the one you talk to, speaks for the herd
- **Bia** — looks things up
- **Nina** — writes and edits files
- **Zeca** — checks the work
- **Joca** — runs commands
- **Duda** — tidies and organises

**Zeca is the one who raises the sign.** Verification isn't a gate bolted on at the end — it's a member of the team.

Four states, always visible: **Dozing · Listening · Working · Needs you.**

## Still worth using

The failure mode of a safety product is that it becomes an obstacle people route around. So the guard rails are aimed narrowly on purpose:

- **Reading is never interrupted.** Silent tier, always. Only actions that change something or leave the machine can pause.
- **Nothing is blocked that you didn't ask for.** The gate pauses the agent, not you.
- **Saying no is cheap.** "Hold on" is the default action, and stopping costs you nothing and is instantly reversible.
- **The interruption is where the value is.** When it asks, it tells you what it is about to do, why it thinks that, and whether it can be undone — in plain words, with no tool identifiers.

The goal is an agent you can leave running, not one you have to babysit.

## What this is NOT

Being honest about limits is more useful than a feature list:

- ❌ **Not a general-purpose autonomous agent.** It asks a lot. That is the point.
- ❌ **Not free of risk.** Nothing that can act on your behalf can be risk-free. The goal is that failure is **small, visible, and reversible** — not that failure is impossible.
- ❌ **Not a chatbot.** If you want a chat window, plenty of those exist.
- ❌ **Not finished.** See status above.
- ❌ **Not affiliated with the OpenClaw Foundation.** See attribution below.

## The acceptance test

This project has exactly one defining test, and the whole product is judged against it:

> **Reproduce the Replit incident, and confirm Capybaras halts at the approval gate and cannot proceed without a human click.**

If Capybaras would have stopped that database deletion, the product works. If it would not, nothing else about it matters.

## Built on OpenClaw

Capybaras is a desktop application built on [OpenClaw](https://github.com/openclaw/openclaw), which is MIT licensed. Its licence and third-party notices are bundled here:

- [`LICENSES/openclaw-MIT.txt`](./LICENSES/openclaw-MIT.txt) — OpenClaw Foundation, MIT
- [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md)

Capybaras also reuses OpenClaw's control interface and gateway. Those components remain under the Foundation's MIT terms; this repository's own code is licensed separately, below.

"OpenClaw" is the trademark of the OpenClaw Foundation. This project is an independent application and is not endorsed by or affiliated with the Foundation.

## Licence

[Apache-2.0](./LICENSE) for this project's own code — chosen over MIT for the explicit patent grant, which matters for software people are asked to trust with file access.

## Project documents

| File | What it covers |
|---|---|
| [`STATUS.md`](./STATUS.md) | Current phase, what's done, what's next |
| [`DECISIONS.md`](./DECISIONS.md) | Why things are the way they are |
| [`BUILD-PLAN.md`](./BUILD-PLAN.md) | Packaging, distribution, licensing, UI spec |
| [`BRAND.md`](./BRAND.md) | The herd, the states, the visual language |
| [`EXECUTION-PLAN.md`](./EXECUTION-PLAN.md) | Build order, workstreams, handoffs |
| [`docs/research/agentic-harm.md`](./docs/research/agentic-harm.md) | The harm classes, and the ten-year estimates cited above |
| [`docs/research/safe-agent-blueprint.md`](./docs/research/safe-agent-blueprint.md) | Can a safe agent exist at all, and where the parts already exist |
| [`docs/plans/M4-visual-design.md`](./docs/plans/M4-visual-design.md) | How the interface becomes beautiful rather than merely functional |
| [`docs/papers/ai-in-defensive-cybersecurity.pdf`](./docs/papers/ai-in-defensive-cybersecurity.pdf) | **The paper** — AI in defensive cybersecurity and threat detection, and why autonomous response is the part that needs a gate |
| [`docs/papers/ai-quantum-and-harm.pdf`](./docs/papers/ai-quantum-and-harm.pdf) | **The synthesis** — 48 pages: AI capability, quantum computing, the harm record, the brain, defensive security, and a unified Bayesian model. Parts in [`docs/papers/synthesis/`](./docs/papers/synthesis/) |

## Sponsors

Free, and staying free. Donations cover code-signing and distribution, which is roughly **$20–150 per year**.

*(GitHub Sponsors link to come.)*

---

*It will still break. It will break small, visibly, and you'll be able to undo it.*
