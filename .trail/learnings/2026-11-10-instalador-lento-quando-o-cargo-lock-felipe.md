---
title: Instalador lento quando o Cargo.lock muda
author: Felipe
date: 2026-11-10
branch: main
cost: { minutes: 1, tokens: 45902 }
touches:
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-db/src/ajustes.rs
  - crates/nfse-db/src/dps.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/reconciliacao.rs
  - crates/nfse-nacional/src/lib.rs
  - auditoria-ui.md
  - tmp-n\303\243o-commitar/dps.xml
  - tmp-n\303\243o-commitar/emitida-pelo-portal-nacional.xml
  - Cargo.lock
---

## Decided
- A lentidão do workflow Instalador (cerca de 28 min na v1.18.0, contra cerca de 8 min) vinha do cache parcial do rust-cache, não do workflow. O hash do Cargo.lock entra na chave, então um lock alterado faz o log mostrar `full match: false` e as dependências são recompiladas.
- Quando o Cargo.lock mudar: enviar a main, esperar o aquecimento do Instalador terminar e só depois enviar a tag `vX.Y.Z`.

## Tried and ruled out
- Mudar o workflow: não precisa. Depois que a main grava o cache do lock novo, as tags seguintes voltam ao tempo normal.

## Watch out
- Se a main e a tag forem enviadas juntas, as duas execuções compilam do zero em paralelo, disputam os runners de macOS e a tag não aproveita nada.
- O macOS Intel é compilado de forma cruzada no runner ARM e é o job mais lento; como a Release espera todas as plataformas, ele define o tempo total.
- Para diagnosticar: `gh run view <id> --log | grep "full match"`.
