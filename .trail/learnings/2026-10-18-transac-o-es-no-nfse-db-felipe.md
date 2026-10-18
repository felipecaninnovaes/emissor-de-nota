---
title: "Transações no nfse-db: conferência e escrita juntas"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 96, tokens: 378991 }
touches:
  - crates/app/src/comandos/tomadores.rs
  - crates/app/ui/app.js
  - crates/nfse-core/src/valor.rs
  - crates/nfse-db/src/dps.rs
  - crates/nfse-db/src/empresas.rs
  - crates/nfse-db/src/lib.rs
  - crates/nfse-db/src/recorrencias.rs
  - crates/nfse-db/src/servicos.rs
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/importacao.rs
---

## Decided
- Conferências que protegem uma escrita (envios pendentes ao trocar ambiente/série ou excluir empresa, maior número usado em `definir_proximo_numero`, empresa lida em `reservar_dps`) ficam dentro da mesma transação da escrita. A auditoria dessas ações usa `auditar_em(&mut tx, ...)` (em `nfse-db/src/lib.rs`) antes do commit.
- UPDATE de `servico` e `recorrencia` confere `empresa_id` no WHERE e não altera o dono; id de outra empresa devolve `NaoEncontrado`.
- `salvar_tomador_vinculado` grava pessoa, vínculo, e-mails e retenção numa transação (o comando `salvar_tomador` do app usa ela).

## Watch out
- O pool do banco tem `max_connections(1)`: dentro de uma transação, chamar `self.empresa()` ou qualquer método que use `self.pool()` trava esperando a conexão. Leia pela transação (`empresa_em(&mut tx, id)`, helpers que recebem `&mut Transaction` ou `SqliteExecutor`).
- Ao montar UPDATE e INSERT com os mesmos binds, use parâmetros numerados (`?1..?N`) nos dois.
