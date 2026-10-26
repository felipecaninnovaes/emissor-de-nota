---
title: Mapa da cobertura de testes do workspace
author: Felipe
date: 2026-10-26
branch: main
cost: { minutes: 2, tokens: 52881 }
touches:
  - Cargo.lock
  - crates/nfse-emissor/src/testes.rs
  - exemplos/dps-exemplo.json
---

## Decided
- O fluxo de emissão (reserva, envio, timeout, reconciliação, cancelamento, recorrência, planilha) é testado em `crates/nfse-emissor/src/testes.rs` (38 testes com Sefin simulada); `lib.rs`, `montagem.rs`, `reconciliacao.rs`, `cancelamento.rs` e `copia.rs` não têm `#[test]` próprio porque são cobertos por lá.
- O XSD oficial (NT 004 e NT 009) é validado em `nfse-xml/src/dps.rs::valida_no_xsd_oficial` e `evento.rs`, ignorados localmente e rodados no CI pelo job xmlsec1 (`--include-ignored`).

## Tried and ruled out
- Contar `#[test]` por arquivo para achar lacunas engana: arquivos com zero testes (lib.rs do emissor, montagem) estão cobertos indiretamente.

## Watch out
- Os casos do XSD partem de `exemplos/dps-exemplo.json` (Simples, ISS não retido, sem retenções federais, tomador CNPJ). Retenções (vRetCP/IRRF/CSLL, tpRetPisCofins), PIS/COFINS próprios, ISS retido, tomador CPF, descontos e regime fora do Simples não passam pelo xmllint.
- Os comandos Tauri (`crates/app/src/comandos/`) e a interface JS não têm testes; o registro no `generate_handler!` não é conferido por nenhum teste.
