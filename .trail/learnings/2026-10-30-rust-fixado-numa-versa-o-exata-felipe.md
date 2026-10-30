---
title: "Rust fixado numa versão exata, igual para ambiente local, CI, releases e imagem do servidor"
author: Felipe
date: 2026-10-30
standing: true
id: l-55b7448a
scope: repo
status: active
src: init-sessions
touches:
  - rust-toolchain.toml
---

## Decided
- Versão flutuante tornava builds irreproduzíveis. A imagem do servidor seguia cada versão nova do Rust.
- In the code: Ao trocar o Rust, atualize rust-toolchain.toml, `dtolnay/rust-toolchain@` nos workflows e servidor/Dockerfile (hoje rust:1.97.1-bookworm).

_From past Claude sessions here, Oct 30._
