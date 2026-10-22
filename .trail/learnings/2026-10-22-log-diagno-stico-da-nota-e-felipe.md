---
title: "Log, diagnóstico da nota e coluna nova em dps"
author: Felipe
date: 2026-10-22
branch: main
cost: { minutes: 11, tokens: 126324 }
touches:
  - AGENTS.md
  - crates/app/src/comandos/mod.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/configuracoes.js
  - crates/app/ui/js/notas.js
  - crates/app/ui/js/util.js
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/testes.rs
  - docs/arquitetura.md
---

## Decided
- Log com `tracing` em `crates/app/src/registro.rs` (arquivo diário, 14 dias, filtro só nos crates do emissor); modo detalhado é um arquivo-marca `log-detalhado` na pasta de dados, pois o log sobe antes do banco abrir.
- A resposta da Sefin fica em `dps.resposta_sefin`; o XML enviado já estava em `dps.xml_assinado`. `Emissor::diagnostico_nota` monta o texto com dados pessoais mascarados (`ELEMENTOS_MASCARADOS`).

## Tried and ruled out
- Migração `0010` para a coluna nova de `dps`: o ajuste `dps_unica_por_ambiente` recria a tabela depois das migrações e descarta a coluna (testes falham com "no such column"). Coluna nova de `dps` vai em `ajustes.rs`, idempotente.

## Watch out
- Elemento novo da DPS com dado pessoal precisa entrar em `ELEMENTOS_MASCARADOS`, senão vaza no diagnóstico copiado.
