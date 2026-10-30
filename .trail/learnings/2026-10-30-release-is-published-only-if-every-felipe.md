---
title: Release is published only if every platform build succeeds
author: Felipe
date: 2026-10-30
standing: true
id: l-3ae5cef9
scope: repo
status: active
src: init-review
---

## Decided
- The installer workflow builds Windows, macOS and Linux separately, and one combined update manifest is needed for auto-update. Publishing a partial set would break updates for some users.
- In the code: In the Instalador workflow, the `publicar` job runs only on tags and needs all matrix builds. It merges them into `latest.json` and creates the Release. Do not publish per platform.

_From PR #6 here, Oct 18._
