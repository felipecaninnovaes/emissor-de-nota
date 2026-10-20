---
title: "Retenções do serviço: chave aplicar dentro de retencoes"
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 1, tokens: 27766 }
touches:
  - Cargo.lock
  - crates/app/ui/js/servicos.js
  - CHANGELOG.md
  - crates/app/Cargo.toml
  - crates/nfse-db/src/servicos.rs
---

## Decided
- `tributacao.retencoes` do serviço mistura percentuais (irrf, csll, cp, pis, cofins) com o modo `aplicar` ("sempre" | "so_tomador_que_retem"). A lista de serviços (crates/app/ui/js/servicos.js) filtra pelas chaves de `NOME_RETENCAO` antes de montar o resumo.

## Tried and ruled out
- Nenhuma mudança no Rust (`RetencoesPercentuais` em crates/nfse-db/src/servicos.rs): o formato salvo está certo, o erro era só de exibição.

## Watch out
- Ao iterar `retencoes` com `Object.entries`, a chave `aplicar` aparece como "undefined so_tomador_que_retem%". Filtre as chaves de percentual.
