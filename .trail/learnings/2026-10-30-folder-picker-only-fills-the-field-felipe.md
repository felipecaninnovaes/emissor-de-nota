---
title: Folder picker only fills the field; validation still happens on Save
author: Felipe
date: 2026-10-30
standing: true
id: l-ebafdc4a
scope: repo
status: active
src: init-review
touches:
  - "crates/app/src/comandos/dialogo.rs#escolher_pasta"
---

## Decided
- The picker is a convenience, so cancelling must not change the value. The chosen folder goes through the same validation as typed paths.
- In the code: `escolher_pasta` in `comandos/dialogo.rs` only returns a path. Validation stays in `definir_pasta` on Save. It is called from Rust, so no new capabilities permissions are needed.

_From PR #7 here, Oct 18._
