---
title: "Mac builds are unsigned, and only the Linux AppImage updates itself"
author: Felipe
date: 2026-10-30
standing: true
id: l-60686585
scope: repo
status: active
src: init-review
touches:
  - docs/deploy.md
---

## Watch out
- The team shipped macOS and Linux installers without Apple signing or notarization. Users must confirm past Gatekeeper, and DEB installs don't self-update.
- In the code: Don't assume signed macOS artifacts or auto-update for DEB. `docs/deploy.md` documents this. Adding notarization would need workflow changes.

_From PR #6 here, Oct 18._
