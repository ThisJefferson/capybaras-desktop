# "What this can do" — the page inside the app

**(Gap 7.3, `docs/plans/fix-the-gaps-2026-09-29.md`.)**

**Why this page exists.** TunnelBear's trust comes from published security audits. Ours currently comes from a threat model, a test suite and a CI badge — **every one a developer surface a user will never open.** The finding in the design comparison was blunt: *we are asking for more trust than a VPN and offering less visible proof.*

**We already have the better material.** The threat model's own *"what we deliberately do not defend against"* section is more honest than anything a commercial VPN publishes. It is simply filed where no user will ever read it.

**This is that content, rewritten for a person.** Not a summary of the threat model — the same honesty, in the second person, at a length someone will actually finish.

**Where it lives:** a page in the app, reachable from the herd screen. Not a modal, not a first-run wall — something a person can find when they wonder *"should I trust this?"*, and read in under a minute.

**How it stays true:** the claims below are the product's public promises. Where the code changes what is true, **this page changes in the same commit.** A page that overstates is the one failure this whole product exists to avoid.

---

## The page, as a person reads it

### What Capybaras can do

Capybaras works on your computer on your behalf. It can read files, write files, run commands, and talk to the model you have connected.

It is an **agent** — it does things, rather than only answering questions. That is what makes it useful, and it is why it asks.

### What Capybaras cannot do

**It cannot act without asking.** Anything that changes something, spends something, or reaches outside your computer stops and waits for you. There is no setting that turns this off, and **no past approval ever stands in for a fresh one on something that cannot be undone.**

**It cannot hide what it is about to do.** When it asks, it shows you **every** reason it stopped — not the most convenient one.

**It cannot keep your key.** The key you connect stays in your computer's own credential store. Capybaras reads it to make the call; it never writes it to a file.

### What it will always ask about

Every time. Not once, not "for this folder", not "from now on":

- **Replacing a file that already exists** — and the card names what would be replaced.
- **Anything that spends money.**
- **Anything that reaches outside your computer.**
- **Anything that cannot be undone.**
- **Anything that changes your security settings.**

### What Capybaras does *not* promise

This is the part most products leave out, and it is the part worth trusting.

**It cannot make your computer clean.** If something harmful is already running on your machine, we can notice it and tell you. We cannot remove it.

**It cannot stop you.** If you deliberately approve something destructive, it happens. Capybaras is not a nanny: it makes sure your decision is **informed**, not that it is **wise**.

**It cannot make the model right.** The gate stops unauthorised action. It does not make the model correct — it can still be wrong, and a card can describe a mistake perfectly accurately.

**It cannot defend against an administrator.** Anyone with administrator rights can change any app on your machine, including this one. We can detect tampering. We cannot prevent it.

### The promise, in one line

> **Capybaras makes sure a destructive action does not run without a human agreeing — and it does not claim anything more than that.**

### And when it is wrong

It will still break. When it does, the aim is that it breaks **small, visibly, and in a way you can undo.**

That is the goal. It is not a guarantee, and we are not going to pretend it is one.

---

## Notes for whoever builds this screen

**No jargon.** The same rule as the card (`src/policy/card-copy.ts`): describe what happens, never how we classify it.

**No mascot on this page.** The herd may signal — this page is not a status display, it is a document. *(See card spec §6.1 for the rule: signal, never opine.)*

**Say the uncomfortable parts first.** "It cannot make your computer clean" is the sentence that earns the rest of the page. Buried at the bottom with the other fine print, it earns nothing.

**One page.** If it needs scrolling twice, it has become a policy document, and the person has stopped reading.

<!-- project: path:C:\Users\Skept\.openclaw\workspace -->
