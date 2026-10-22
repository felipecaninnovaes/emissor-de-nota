---
title: "E1235: finNFSe agora é filho direto de infDPS (NT 009)"
author: Felipe
date: 2026-10-22
branch: main
cost: { minutes: 3, tokens: 57312 }
touches:
  - Cargo.lock
  - crates/nfse-pdf/src/dados.rs
  - crates/nfse-xml/src/dps.rs
  - docs/fiscal.md
---

## Decided
A Sefin passou a validar a DPS pelo leiaute da NT 009: `finNFSe` saiu do grupo `IBSCBS` e é filho obrigatório de `infDPS`, entre `tpEmit` e `cLocEmi`. O gerador (`nfse-xml/src/dps.rs`) emite `finNFSe=0` sempre ali. O PDF lê nos dois lugares (notas antigas têm em `IBSCBS`).

## Tried and ruled out
Mexer na ordem de `cLocEmi`: ele já estava certo, o erro E1235 ("esperado cMotivoEmisTI, finNFSe") só apontava o primeiro elemento depois do que faltava. Baixar o XSD novo: o portal gov.br só lista o pacote da NT 004; o NT 009 só existe em PDF/planilha.

## Watch out
O XSD versionado em `xsd/` é o antigo, então os testes `#[ignore]` com xmllint recusam a DPS nova até o pacote ser atualizado. A posição exata de `finNFSe` vem do texto do NT 009 e da mensagem da Sefin, não do XSD; se vier novo E1235, comparar com o esquema publicado.
