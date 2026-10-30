---
title: O projeto agora tem servidor de backup que precisa de Postgres real nos testes
author: Felipe
date: 2026-10-30
standing: true
id: l-d3d5efdf
scope: repo
status: active
src: init-sessions
touches:
  - compose.yaml
---

## Decided
- O remoto trouxe servidor/, projeto Rust separado. A premissa de que não havia serviços nem Docker em dev deixou de valer, e o compose.yaml foi mantido.
- In the code: Use `devbox run up` e `devbox run down` (compose.yaml) para o Postgres antes dos testes do servidor/. `devbox run test` cobre o workspace e o servidor.

_From past Claude sessions here, Oct 30._
