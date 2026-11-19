---
title: Viabilidade de cancelamento em lote com aviso por e-mail
author: Felipe
date: 2026-11-19
branch: main
cost: { minutes: 1, tokens: 50208 }
touches:
  - crates/nfse-emissor/src/cancelamento.rs
---

## Decided
Só avaliação, nada implementado. É feature: passa por /spec. Reaproveita `Emissor::cancelar` (crates/nfse-emissor/src/cancelamento.rs), uma nota por vez, com o mesmo motivo e a mesma justificativa para todas, e `Emissor::enviar_email` (envio_email.rs), que já anexa o PDF com a tarja Cancelada quando `dados.cancelada`.

## Tried and ruled out
Cancelar o lote numa chamada só ao backend: fica melhor a tela chamar nota a nota e mostrar o andamento, como a emissão por planilha faz.

## Watch out
O assunto e o corpo do e-mail (ConfigSmtp) são os da emissão; o cancelamento precisa de texto próprio. "Notas emitidas" (ui/js/notas.js) não tem seleção múltipla. Falha no e-mail não pode desfazer nem bloquear o cancelamento. Em Produção, o cancelamento é feito só pelo usuário.
