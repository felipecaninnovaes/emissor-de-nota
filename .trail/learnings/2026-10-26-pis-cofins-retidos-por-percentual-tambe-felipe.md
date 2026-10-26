---
title: PIS/COFINS retidos por percentual também levam vPis/vCofins
author: Felipe
date: 2026-10-26
branch: main
cost: { minutes: 3, tokens: 58262 }
touches:
  - CHANGELOG.md
  - Cargo.lock
  - crates/nfse-emissor/src/montagem.rs
  - crates/nfse-emissor/src/testes.rs
  - crates/nfse-xml/src/dps.rs
  - docs/fiscal.md
---

## Decided
`federais_da_nota` (nfse-emissor/src/montagem.rs), no caminho por percentuais de retenção (`retencoes.pis/cofins`), passa a preencher vBCPisCofins = valor da nota, alíquotas da retenção e vPis/vCofins com os mesmos valores (arredondados) somados em vRetCSLL. Motivo: o DANFSe mostrava "PIS/COFINS - Débito Apuração Própria" zerados. Tributo não retido usa a alíquota própria truncada.

## Tried and ruled out
A regra antiga (retido vai só em vRetCSLL, sem vPis/vCofins, para não reduzir a base do IBS/CBS "duas vezes") foi revertida: vRetCSLL não mexe na base do IBS/CBS e o Portal Nacional manda vPis/vCofins junto com o tipo 3.

## Watch out
Com vPis/vCofins a Sefin reduz a base do IBS/CBS (na nota de 64.715,35: de 62.773,89 para ~60.411,78). Nada validado com emissão real até 26/10/2026. Valor retido é arredondado e o próprio é truncado: podem diferir 1 centavo.
