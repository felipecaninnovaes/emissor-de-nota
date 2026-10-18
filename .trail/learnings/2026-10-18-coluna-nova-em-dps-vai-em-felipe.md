---
title: "Coluna nova em dps vai em ajustes.rs, não em migração"
author: Felipe
date: 2026-10-18
branch: correcoes-revisao
cost: { minutes: 11, tokens: 140602 }
touches:
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-db/src/ajustes.rs
  - crates/nfse-db/src/dps.rs
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/testes.rs
  - crates/nfse-nacional/src/lib.rs
  - crates/nfse-xml/src/evento.rs
  - docs/arquitetura.md
  - docs/desenvolvimento.md
---

## Decided
- `dps.enviado_em` (base da janela de 5 min da reconciliação, `JANELA_RECONCILIACAO`) foi criada em `crates/nfse-db/src/ajustes.rs` (`dps_enviado_em`, idempotente), não como migração 0007. O mesmo ajuste recria o índice `dps_empresa_id` da migração 0005.
## Tried and ruled out
- Migração `0007_dps_enviado_em.sql` com ALTER TABLE: num banco novo o ajuste `dps_unica_por_ambiente` roda depois de TODAS as migrações e recria `dps` com a lista fixa de colunas, apagando a coluna (testes deram "no such column: enviado_em"). Pelo mesmo motivo, bancos novos perdiam o índice `dps_empresa_id`.
## Watch out
- Qualquer coluna ou índice novo em `dps` tem de ir em ajustes.rs depois de `dps_unica_por_ambiente`. A próxima migração continua sendo a 0007.
- Depois de mexer em migrações, `cargo clean -p nfse-db` se o sqlx::migrate! não pegar a mudança.
