---
title: "Fase 2 da auditoria: nenhum caminho libera DPS sem prova da Sefin"
author: Felipe
date: 2026-11-10
branch: main
cost: { minutes: 22, tokens: 229337 }
touches:
  - crates/nfse-db/src/ajustes.rs
  - crates/nfse-db/src/dps.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/reconciliacao.rs
  - crates/nfse-emissor/src/testes.rs
  - docs/arquitetura.md
  - auditoria-ui.md
  - tmp-n\303\243o-commitar/dps.xml
  - tmp-n\303\243o-commitar/emitida-pelo-portal-nacional.xml
  - CHANGELOG.md
---

## Decided
- Nenhum caminho libera DPS sem prova da Sefin. Só o 404 com JSON da Sefin e código E2404 prova que a DPS não gerou nota (`resultado_consulta_dps` em nfse-nacional). Só 4xx com mensagens é `Rejeitada`; 5xx vira `Http` e segue o caminho incerto.
- Chave da NFS-e autorizada e não gravada fica em `dps.chave_recebida` (coluna via `ajustes.rs`, não migração). A reconciliação busca por ela (`GET /nfse/{chave}`) e `liberar_para_reenvio` recusa DPS com chave.
- `gravar` exige `id_dps` do `infDPS` da NFS-e igual ao da DPS; NFS-e de outra DPS deixa a DPS incerta sem guardar a chave.
- Depois do envio, `ErroEmissao::Banco` vira `ResultadoEmissao::Incerta` ("use Verificar"), em `Emissor::desfecho`.
- `registrar_envio` só aceita DPS `reservada`; rejeitada não volta à Sefin (nova tentativa reserva outro número).
- A reserva confere ambiente e série contra a empresa lida na validação.

## Tried and ruled out
- Exigir corpo da Sefin no 404 da consulta de eventos: formato desconhecido, quebraria a sincronização com o portal, e 404 falso ali não duplica nota (cancelamento é idempotente).
- Teste da troca de ambiente no meio da emissão: `Conector::conectar` é síncrono e não consegue mexer no banco no meio.

## Watch out
- O formato real do 404 de `GET /dps/{id}` (JSON com E2404) foi deduzido de `mensagens.rs`; conferir com `nfse-poc consultar-dps` num número inexistente. Se vier sem E2404, nenhuma DPS incerta é liberada e "Consultar na Sefin" da numeração falha.
- Na `SefinSimulada`, a NFS-e com `<infDPS Id="X">` sai com o Id da última DPS enviada ou consultada; use outro Id para simular NFS-e de outra DPS. `fechar_no_envio` fecha o banco no envio para simular falha do banco.
