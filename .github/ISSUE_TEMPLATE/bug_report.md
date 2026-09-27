---
name: Bug report
about: Something is wrong with Capybaras
title: ""
labels: bug
assignees: ""
---

**Describe the bug**

A clear and concise description of what is happening that should not be happening, or what is not happening that should.

**To reproduce**

Steps to reproduce the behaviour:

1. Start Capybaras with '...'
2. Ask Tuca to '...'
3. Wait for the approval gate
4. See the error

**Expected behaviour**

What you expected to happen instead.

**Actual behaviour**

What actually happened. Include any error messages, unexpected approvals, or silent failures.

**Screenshots or logs**

If applicable, add screenshots or paste the relevant log output. Capybaras logs are in `[PLACEHOLDER: log directory]`.

**Environment**

- OS and version: [e.g. Windows 11 23H2]
- Capybaras version: [e.g. commit hash or tag, run `git describe --tags` if you have the repo]
- Node version (if building from source): `node --version`
- Rust version (if relevant): `rustc --version`

**Risk classifier context**

If the bug involves an incorrect classification (too strict, too permissive), tell us what the action was, what classification you think it deserved, and what it actually got. The classifier's output is logged.

**Additional context**

Anything else that might help. How often does this happen? Does it happen with every action of this type, or intermittently?

**Checklist before submitting**

- [ ] I have checked the existing issues for duplicates
- [ ] I have included logs or screenshots if the issue is visual
- [ ] I have described any safety concern, even if vague