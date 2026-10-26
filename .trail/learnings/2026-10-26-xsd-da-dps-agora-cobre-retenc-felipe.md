---
title: "XSD da DPS agora cobre retenções, CPF, descontos e não optante"
author: Felipe
date: 2026-10-26
branch: main
cost: { minutes: 2, tokens: 35210 }
touches:
  - Cargo.lock
  - crates/nfse-xml/src/dps.rs
---

## Decided
- `valida_no_xsd_oficial` (nfse-xml/src/dps.rs) passou a validar 13 casos nos dois leiautes: além dos anteriores, retenções federais (tpRetPisCofins 3 com vRetCP/IRRF/CSLL), PIS/COFINS próprios no Simples, ISS retido com pAliq, tomador CPF, descontos e não optante com pTotTrib e com vTotTrib. Isso substitui o "watch out" do mapa de cobertura de 26/10 sobre essa lacuna.

## Tried and ruled out
- Montar os casos por JSON novo em `exemplos/`: preferido derivar de `exemplo()` no próprio teste, como os casos de obra e ente público.

## Watch out
- Os XMLs ficam em `%TEMP%/nfse-xml-xsd/dps-N.xml`; o último leiaute do laço (NT 004) sobrescreve o NT 009. Para inspecionar, abra-os depois de rodar o teste.
- `trail check` acusa falsos conflitos nesse teste por palavras como "sem tomador" e "cpf documento"; o teste só usa os tipos existentes.
