---
title: Todos os pacotes do ambiente de dev devem vir do mesmo commit do nixpkgs
author: Felipe
date: 2026-10-30
standing: true
id: l-6b0d92c3
scope: repo
status: active
src: init-sessions
touches:
  - devbox.json
  - devbox.lock
---

## Watch out
- Gcc e libclang de commits diferentes do nixpkgs trouxeram glibcs diferentes, e o build do SQLite embutido quebrou com símbolo indefinido.
- In the code: Ao mudar um pacote em devbox.json, confira no devbox.lock se gcc e libclang seguem no mesmo commit do nixpkgs. Sintoma da falha: `undefined symbol: __pointer_chk_guard` no build do libsqlite3-sys. Adicione pacotes via `devbox add`, não editando o JSON.

_From past Claude sessions here, Oct 30._
