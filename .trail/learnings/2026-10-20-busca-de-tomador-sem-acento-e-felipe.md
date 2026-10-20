---
title: Busca de tomador sem acento e valor limpo após emitir
author: Felipe
date: 2026-10-20
branch: main
cost: { minutes: 3, tokens: 26016 }
touches:
  - crates/app/ui/js/emitir.js
  - crates/nfse-db/src/tomadores.rs
---

## Decided
- A busca de tomadores (`buscar_tomadores` e `buscar_tomadores_de_outras_empresas` em nfse-db/src/tomadores.rs) filtra por nome em Rust (`filtrar_por_termo`, `sem_acentos`), sem diferenciar maiúsculas nem acentos. O SQL só traz os tomadores da empresa, ordenados por nome. Documento continua "começa com", com ou sem pontuação. Limite de 20.
- Após autorizar a nota, a tela limpa também `#valor` antes de `aplicarServico()` (emitir.js); se o serviço tem valor sugerido, ele volta preenchido.

## Tried and ruled out
- `LIKE ... COLLATE NOCASE` do SQLite: só ignora maiúsculas ASCII, então "saude" não acha "Saúde". Não resolve acento.

## Watch out
- `tomadores.rs` está em CRLF; scripts que editam o arquivo devem preservar as quebras de linha.
- Se a lista de acentos em `sem_acentos` não cobrir um caractere, ele passa como está.
