# Protecting the policy file with an ACL

**Audience: an operator with administrator rights.** Ten minutes.

---

## Why this is needed

The protection policy — freezes and off-limits targets — decides what the agent is not allowed to do. It lives in a file. **The agent runs as the same user account as the app**, which means:

> If that account can write the policy file, **the agent can rewrite its own freeze.**

There is no code in this repository that can prevent that. The app can only *detect* it and say so at startup. **Enforcement requires the operating system.**

That is what this document sets up: a machine-wide policy file the user account can read but not modify.

---

## Where the app looks

The sidecar resolves the policy in this order, and uses the first that exists:

1. **`%ProgramData%\Capybaras\policy.json`** — machine-wide, and the one you can protect. **This is what the ACL below applies to.**
2. `%LOCALAPPDATA%\Capybaras\policy.json` — the per-user state directory. Convenient, and **not enforceable.**

At startup the sidecar logs which one it is using and warns if it is writable by the current account. If you see that warning, this document is the fix.

---

## The commands

**Run every one of these in an Administrator PowerShell.** Without elevation they will fail partway, which is worse than not starting.

### 1. Create the directory

```powershell
New-Item -ItemType Directory -Force -Path "$env:ProgramData\Capybaras"
```

### 2. Stop inheriting permissions from ProgramData

By default a new folder inherits permissions that may grant the user write access. Remove the inheritance first — this is the step that is easy to skip and that silently undoes everything else.

```powershell
icacls "$env:ProgramData\Capybaras" /inheritance:r
```

### 3. Administrators full, ordinary users read-only

```powershell
icacls "$env:ProgramData\Capybaras" /grant "*S-1-5-32-544:(OI)(CI)F"
icacls "$env:ProgramData\Capybaras" /grant "*S-1-5-32-545:(OI)(CI)R"
```

`*S-1-5-32-544` is the built-in **Administrators** group and `*S-1-5-32-545` is **Users**. They are written as SIDs rather than names on purpose: **group names are localised.** On a non-English Windows `BUILTIN\Administrators` is spelled differently, and a command that fails on a localised system is a command that gets skipped. SIDs work everywhere.

- `(OI)(CI)` — object inherit and container inherit, so the settings apply to files created inside.
- `F` = full control, `R` = read and execute.

### 4. Write the policy

Create the file **as an administrator**:

```powershell
@'
{
  "freeze": {
    "active": true,
    "reason": "declared change freeze",
    "declaredBy": "you",
    "declaredAt": "2026-09-27"
  },
  "protectedTargets": ["*production*", "*invoice*"]
}
'@ | Set-Content -Path "$env:ProgramData\Capybaras\policy.json" -Encoding UTF8
```

### 5. Lock the file down

The directory permissions cover files created *after* step 3, but the file itself may still carry inherited entries. Be explicit:

```powershell
icacls "$env:ProgramData\Capybaras\policy.json" /inheritance:r
icacls "$env:ProgramData\Capybaras\policy.json" /grant "*S-1-5-32-544:F"
icacls "$env:ProgramData\Capybaras\policy.json" /grant "*S-1-5-32-545:R"
```

---

## Verify it worked

**1. Read the ACL:**

```powershell
icacls "$env:ProgramData\Capybaras\policy.json"
```

You want to see **Administrators:(F)** and **Users:(R)** — and **no (W), (M), or (F) entry for your own account or for Users.**

**2. Prove you cannot write it.** This is the test that matters, and it should **fail**:

```powershell
"tampered" | Out-File "$env:ProgramData\Capybaras\policy.json" -Append
```

Expected: **Access to the path is denied.** If that command succeeds, the protection is not in place — go back to step 2, because inheritance is almost always the cause.

**3. Confirm the app agrees.** Start Capybaras and read the log. You should see the machine-wide path in use, and **no** "writable by this account" warning. If the warning still appears, the app is reading the per-user copy — check that `%ProgramData%\Capybaras\policy.json` actually exists as an administrator, because step 1 of the resolution order skips a path that is not there.

---

## What this does and does not achieve

**Does:** a user-level process — including the agent — can no longer modify the freeze, the off-limits list, or the protected tools. Modifying them requires elevation.

**Does not:**
- Stop an attacker who already has **administrator** rights. Nothing at the application layer can.
- Stop the `grants.json` file from being tampered with. Grants fail closed and are re-validated on load, but they live in the per-user state directory and are not covered by this document. That is the next candidate for the same treatment.
- Make the policy *trustworthy*, only **unmodifiable**. Nothing verifies who wrote it. Signing is M6 work.

---

## To undo it

```powershell
icacls "$env:ProgramData\Capybaras" /inheritance:e
Remove-Item "$env:ProgramData\Capybaras\policy.json"
```

The app will fall back to the per-user location, and will say so at startup.
