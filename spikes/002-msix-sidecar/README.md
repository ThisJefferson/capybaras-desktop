# Spike 002 — MSIX full-trust + bundled Node sidecar

## Verdict: VALIDATED (with two caveats that change how the Gateway must be written)

**Question:** does a bundled Node sidecar work inside an MSIX full-trust package? This was the highest-risk unverified assumption inherited from the packaging research (`DECISIONS.md` D11) — no single source documented MSIX + a desktop shell + a Node sidecar together.

**Answer: yes.** A full-trust MSIX app with package identity launched, resolved its own app-local `node.exe`, spawned it, and Node ran. Evidence below is from the app's own output, not inference.

---

## Evidence

Built a real package: a stdlib-only Rust launcher (`launcher.exe`, 247 KB) that spawns a bundled `node.exe` (88 MB) running a bundled `sidecar.mjs`, packed with `makeappx` into a 34 MB MSIX.

Launcher output, captured from inside the package context:

```
== full-trust MSIX launcher ==
launcher exe     : ...\package\launcher.exe
app dir          : ...\package
node.exe present : true
sidecar present  : true
app dir writable : yes
node spawn       : OK (exit code: 0)
--- node stdout ---
sidecar ran under full-trust MSIX: v24.16.0
  execPath: ...\package\node.exe
  cwd     : C:\Windows\system32
```

`sidecar-proof.json` confirms it independently: `"ran": true`, `"nodeVersion": "v24.16.0"`, `"execPath": "...\package\node.exe"`, `"platform": "win32"`.

So: **package identity + full trust + app-local spawn of a bundled Node runtime = works.**

---

## Caveat 1 — AppData writes are virtualized (finding, not a blocker)

The launcher wrote to `%LOCALAPPDATA%\CapybarasSpike\`. The files were **not there**. They were in:

```
%LOCALAPPDATA%\Packages\CapybarasSpike_ffssfw5f1446m\LocalCache\Local\CapybarasSpike\
```

Meanwhile the sidecar's own view of `process.env.LOCALAPPDATA` was plain `C:\Users\Skept\AppData\Local`, and `process.env.USERPROFILE` was plain `C:\Users\Skept`.

**The redirection is invisible to the process.** The Gateway will believe it is writing to `%LOCALAPPDATA%\Capybaras\...` while the bytes land in the package's private store. That is *good* for isolation and *bad* for anything that needs to be shared with, or found by, the user:

- Where does the Gateway keep its session database and credentials? Under MSIX it lands somewhere invisible.
- If the user or a support process must find that state, the path is not the one the app reports.
- Any migration or uninstall story has to account for it.

**Design consequence: decide the state location deliberately.** Either accept the virtualized private store (and expose an in-app "open data folder" action), or redirect explicitly to a real, user-visible path.

## Caveat 2 — `cwd` is `C:\Windows\system32`, and the app-dir reading is not representative

Two related traps:

1. **The packaged app launched with `cwd = C:\Windows\system32`.** Any code that resolves files relative to the working directory is broken under MSIX. Paths must come from `process.execPath` / the module URL / an explicit configured root — never from `cwd`.

2. **`app dir writable: yes` is NOT trustworthy as a result.** This test ran from a **loose layout registered out of the workspace folder**, which is writable by definition. A genuinely installed package lives in `WindowsApps`, which is read-only. **The writability reading must be re-taken against a real install** before anyone relies on it. Recorded here rather than quietly dropped.

---

## How it was installed, and the elevation wall

Three attempts, and the middle one is the useful one:

1. **Unsigned MSIX → rejected.** `0x800B0100: No signature was present in the subject.` Developer Mode permits *sideloading*; it does not permit *unsigned*.
2. **Self-signed, cert in `CurrentUser\TrustedPeople` → rejected.** `0x800B0109: The root certificate ... is not trusted by the trust provider.` MSIX deployment wants **machine**-level trust, and importing to `LocalMachine\TrustedPeople` returns `E_ACCESSDENIED` (`0x80070005`) without elevation.
3. **Loose-layout registration → succeeded, no signature, no elevation.** `Add-AppxPackage -Register AppxManifest.xml`. `IsDevelopmentMode = True`. This is the developer workflow and it is what produced the evidence above.

**Distribution consequence:** installing a self-signed MSIX locally needs **one elevated action**. It is *not* needed for the real distribution path — Store packages are re-signed by Microsoft, so end users never touch a certificate. The elevation requirement is a **developer-machine** cost only.

---

## What worked

- `makeappx pack` from a folder layout, straightforwardly.
- `New-SelfSignedCertificate` + `signtool sign` in **user** scope — no admin for cert creation or signing, only for trusting it.
- Loose-layout registration as the fast dev loop (`IsDevelopmentMode = True`).
- Launching via `shell:AppsFolder\<PFN>!App` — worked from this session.
- The bundled Rust launcher, zero dependencies, 247 KB.

## What failed or surprised us

- **Unsigned MSIX is not a thing**, even in Developer Mode. Corrected an assumption I was carrying.
- **User-scope cert trust is insufficient** for MSIX deployment; it must be machine scope.
- **AppData virtualization is silent** (Caveat 1) — the single most consequential discovery here.
- **`cwd` is system32** (Caveat 2) — a live bug waiting to happen in the Gateway.
- The first proof-hunt walked the entire user profile and **timed out at 300 s**. Narrowed to exact paths. Lesson: search the paths you have reason to believe, not the whole disk.

## Recommendation

**Ship MSIX, and encode these three rules in the Gateway before the shell is built:**

1. **Never resolve paths from `cwd`.** Use `process.execPath` and explicit roots.
2. **Decide the state directory deliberately** and surface it in the UI, because MSIX will hide it.
3. **Expect one elevated command on a developer machine.** Document it. Do not build anything that depends on self-signed trust at runtime.

## Next production step

Proceed to **M2, the Tauri shell**, with the Job-Object supervision from spike 001 and these three rules from spike 002. Re-take the app-dir writability reading against a genuinely installed package when packaging work begins — it is the one measurement this spike could not make honestly.

---

## Reproducing

`build.ps1` regenerates the package: rasterises the manifest PNGs from `assets/brand/app-icon.svg` via headless Chrome, builds the launcher with `cargo build --release`, copies in `node.exe`, and packs with `makeappx`. The bundled `node.exe` and the produced `.msix` are gitignored — regenerate rather than commit.
