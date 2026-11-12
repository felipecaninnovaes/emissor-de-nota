---
title: Emissão duplicada por Ctrl+Enter/Revisar durante o envio (item 1 da auditoria)
author: Felipe
date: 2026-11-12
branch: fix/emissao-duplicada-durante-envio
cost: { minutes: 5, tokens: 53910 }
touches:
  - CHANGELOG.md
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/preferencias.js
---

## Decided
Guarda só na interface: `revisar()` (emitir.js) sai cedo com `emissaoEmCurso`; o handler de `#confirmar-emissao` também; `#revisar` fica desabilitado durante o envio e é reabilitado no `finally`; o Ctrl+Enter em preferencias.js ignora `ev.repeat`. Linha no CHANGELOG em Não publicado.

## Tried and ruled out
Trava no backend contra emissão simultânea da mesma empresa ficou de fora desta etapa (proposta como etapa separada, se o usuário quiser).

## Watch out
`descartarRevisao()` reabilita `#revisar` quando ele está desabilitado, mas sai cedo com `emissaoEmCurso`, por isso desabilitar o botão durante o envio não conflita. Os checkboxes de retenção da revisão chamam `revisar()` e, durante o envio, não fazem nada. Mudança só em JS, sem teste automatizado: conferir no app.
