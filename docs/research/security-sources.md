# Security sources

**Compiled:** 2026-09-27 · **Status:** index + curation, not an exhaustive scan

> **Read this caveat first.** I verified the *locations and access* for everything below. I did **not** read forty years of issues and I am not going to pretend I did. Section 2 is therefore a **map of where to look and what to look for**, not a set of found articles — inventing issue numbers and quotes would be worse than admitting the gap. If a genuine issue-by-issue scan is wanted, say so and I will work through a defined range and report what is actually in them.

---

## 1. 2600: A Hacker's Quarterly — the public archive

Founded 1984, still publishing. Two distinct things exist online, and conflating them wastes time:

**(a) The Internet Archive holds substantial scans.** Verified items include:

| Holdings | Coverage | URL |
|---|---|---|
| Individual issues, various | at least Spring 2000 onward | `archive.org/details/2600-2000-spring` |
| Volume 37, Issue 2 | 2020 | `archive.org/details/2600-volume-37-issue-2` |
| Volume 40, Issue 1 | 2023 | `archive.org/details/2600-volume-40-issue-1` |
| Volume 40, Issue 2 | 2023 | `archive.org/details/2600-magazine-2600-the-hacker-quarterly-volume-40-issue-2` |
| Volume, Spring | 2005 | `archive.org/details/2600TheHackerQuarterly2005Spring` |

Collection: `computermagazinesmisc` / `magazine_rack`. **Honest note on completeness:** these are *scattered individual issues*, not a complete run. Coverage looked strongest around 2000 and 2020–2023; I did not establish a contiguous set, and there is no guarantee the gaps are fillable from this source.

**(b) `2600.com` itself is active in 2026.** The site carries an archive section, and its newsletter **Off The Wall** is being archived as it is published — the site listed the 2026-09-15 edition as newly archived. **The primary source is alive**, which matters more than the mirrors: if you want to read current 2600 thinking, go to 2600.com first.

**(c) Off The Hook audio is archived too.** A separate Internet Archive item (`archive.org/details/pcs_2600_The_Hacker_Quarterly`) carries dozens of **Off The Hook** episodes as OGG files, with dates visible in the filenames (e.g. 2005-01-05, 2005-01-19). So the broadcast archive and the print archive are separate collections and both exist.

**One legal wrinkle worth knowing before quoting anything:** Wikipedia records a dispute in which Trunk Archive claimed 2600 material had been released into the public domain, then retracted that and apologised. Treat any claim that 2600 content is freely relicensable with suspicion. **Grade B** — secondary source, single account.

---

## 2. What in 2600 is relevant to this project

**Be clear about the honest split.** Most of 2600's 40+ year history is telephone phreaking, switch hacking, and hardware from a dead era. That material is **irrelevant to a desktop approval gate** and I am not going to pad a list with it.

What *is* relevant — as categories to look for, not as articles I have read:

1. **Social engineering — "hacking the human".** 2600's longest-running practical theme and the closest analogue to our worst design-specific risk (`docs/threat-model.md`, **T4**): an attacker crafting the *text of the approval card* so a person approves something harmful. Any 2600 material on pretexting, authority impersonation, and getting a human to do the thing voluntarily is directly applicable.

2. **Local and physical access.** 2600 has always emphasised that physical access defeats most defences. That is precisely our **T2** — the sidecar bundle is a plain writable file, so "access to the machine" is "control of the gate". Worth reading as a corrective to over-trusting software controls.

3. **Input capture and desktop surveillance.** Keyloggers, screen capture, and "watching the user" — the **T1/T2** attacker model (A2: malware running as the user).

4. **The trust relationship between a person and software acting on their behalf.** This is 2600's oldest cultural thread: suspicion of systems that claim authority. Our product is an agent that acts for you, which makes the question *"why should the user believe the card they are shown?"* a 2600-shaped question.

5. **Disclosure ethics.** 2600's editorial line — that curiosity is not a crime, and that disclosure is a public good — is the cultural context for building a security tool honestly. It is also the reason our own `DEBUGGING.md` and `threat-model.md` state limits rather than implying guarantees.

**Where I would start if doing a real scan:** the social-engineering letters and the "best of" material, rather than chronologically. Chronological reading of a 40-year zine is mostly telephony archaeology.

---

## 3. Books

Curated, with the reason each matters *here*. Most of the obvious "security bookshelf" is absent on purpose — see the rejections at the end.

**Ross Anderson, *Security Engineering: A Guide to Building Dependable Distributed Systems*, 3rd ed. (2020).**
**Chapters are downloadable free** from the author's Cambridge page: `cl.cam.ac.uk/archive/rja14/book.html`. Verified.
*Why here:* the single most relevant book for a product like this, and the only one on the list that is free. It is about building systems that stay dependable **in the face of malice** — not about pentesting. Its framing that security failures are usually *policy, incentive and human* failures rather than cryptographic ones maps directly onto our threat model's conclusion that approval fatigue (T5) is a security property. If one book is read, read this one.

**Adam Shostack, *Threat Modeling: Designing for Security* (2014).**
*Why here:* `docs/threat-model.md` is an instance of this method. Worth reading to check whether ours is shaped correctly, particularly on the discipline of stating what you do **not** defend against — the section most threat models omit and most readers need.

**Michal Zalewski, *The Tangled Web* (2011).**
*Why here:* our frontend is a **webview**, and our threat model rates webview XSS as a single point of failure for the entire product (**T3**). This is the book on why browsers are hard to secure and which assumptions leak. Dated, still the best structural account.

**Christopher Hadnagy, *Social Engineering: The Science of Human Hacking*.**
*Why here:* the systematic taxonomy of manipulation — pretexting, authority, urgency, reciprocity. This is the **T4** and **T5** reading: our card is a conversation with a human, and humans are the last line of defence, so how they are manipulated is a design input rather than a footnote.

**Kevin Mitnick, *Ghost in the Wires* (2011).**
*Why here:* the practitioner's-account companion to Hadnagy. Mitnick's attacks were overwhelmingly *social*, not technical, which is the correct prior for anyone designing an approval gate. Read as evidence for how often the human is the actual path.

**Nicole Perlroth, *This Is How They Tell Me the World Ends* (2021).**
*Why here:* the supply chain (**T10**). Our build depends on npm, cargo, and GitHub Actions — and our workflow currently pins actions by **mutable tag**, which this book is a long argument against.

**Andy Greenberg, *Sandworm* (2019).**
*Why here:* a narrative account of what happens when a destructive action *is* permitted. The failure mode our product exists to prevent, at nation-state scale. Useful for keeping the stakes concrete rather than abstract.

**Joseph Menn, *Cult of the Dead Cow* (2019).**
*Why here:* hacker-culture history and the ethics/disclosure argument, from the same cultural lineage as 2600. Context for section 2's point 5.

**Emmanuel Goldstein (ed.), *The Best of 2600* (2008).**
*Why here:* if section 2 is to be acted on, this is the efficient route — a curated anthology instead of forty years of scans.

**Standards, not books, and arguably more useful than either:** the **OWASP ASVS** (a testable requirements checklist) and the **OWASP Top 10**. ASVS is the one that would actually change our backlog, because it is written as verifiable requirements rather than prose.

### Rejected, and why

- ***The Web Application Hacker's Handbook*** — excellent, but it teaches **finding** web flaws. We are not pentesting a web app; we are building a gate. Wrong direction, and much of it is a decade old.
- **CVE compendia and "hacking exposed" volumes** — no method. A list of specific breakages from last year without a way to reason about next year's.
- **Anything crypto-centred** — we have no interesting crypto problem. Our failures are policy, human and process failures, and a cryptography book would misspend the attention.

---

## 4. Podcasts

**Off The Hook** — 2600's own broadcast; Emmanuel Goldstein and others; long-running, weekly. *Verified archived* (Internet Archive, OGG, 2005-era episodes visible; 2600.com remains active in 2026).
*Good for:* the 2600 voice directly, disclosure ethics, listener questions. The natural companion to section 1, and the fastest way to absorb a 40-year editorial position.

**Darknet Diaries** — Jack Rhysider; narrative, ~monthly.
*Good for:* threat modelling by story. Each episode is a real intrusion, and the recurring lesson is that the technical exploit is rarely the hard part. The best single source for calibrating what actually happens to real organisations.

**Security Now** — Steve Gibson with Leo Laporte; weekly, very long-running.
*Good for:* sustained technical depth and a long memory. Dense, and opinionated in ways worth noticing rather than adopting.

**Risky Business** — Patrick Gray; weekly.
*Good for:* industry and supply-chain reporting, with a sceptical tone. Closest to professional-security practice rather than consumer advice.

**Malicious Life** — Ran Levi; narrative history of malware.
*Good for:* how these failures developed. Useful background for T10 and T2.

**Smashing Security** — lighter, conversational.
*Good for:* accessibility. Honestly more entertainment than reference; include it for the days when "educational" is the barrier.

**Cadence and liveness note:** I did **not** verify current publishing status episode-by-episode in 2026. Off The Hook and Security Now are long-running and were recently active; the remainder I am less certain about. Treat the cadences above as historical rather than as a guarantee about this month.

---

## 5. What I could not verify

Stated so nobody builds on sand:

1. **I did not read any 2600 issues.** Section 2 is a map, not a finding. No issue numbers or quotes are cited because I have none I can stand behind.
2. **The Internet Archive's 2600 coverage is not contiguous.** I confirmed individual issues exist and roughly where the clusters are. I did not establish which years are missing.
3. **Whether archive.org's 2600 uploads are authorised.** The uploads exist and are publicly readable; I did not determine their rights status, and the Trunk Archive episode is a reminder that this is contested.
4. **Current podcast cadences.** See the note in section 4.
5. **`archive.org/details/pcs_2600_The_Hacker_Quarterly`** — I confirmed the item and the Off The Hook audio filenames, but not the full extent of that collection.
6. **This document was compiled without completing the intended research pass.** A research subagent was dispatched for it and returned nothing; the sections above are my own work under time pressure. A second, slower pass would improve sections 2 and 4 in particular.

**The one claim I would least trust:** that section 2's five categories are the *right* five. They are reasoned from our threat model rather than drawn from the material, which is exactly the kind of inference that a real scan could falsify.
