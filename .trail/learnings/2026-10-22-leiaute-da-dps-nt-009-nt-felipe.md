---
title: Leiaute da DPS (NT 009 × NT 004) escolhido por ambiente
author: Felipe
date: 2026-10-22
branch: main
cost: { minutes: 19, tokens: 167755 }
touches:
  - Cargo.lock
  - crates/app/src/comandos/cofre.rs
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/configuracoes.js
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-db/src/lib.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/montagem.rs
---

## Decided
`LeiauteDps` (nfse-core) vai em `Dps.leiaute` (serde default Nt009, não entra no XML). O emissor lê `leiaute_dps_restrita`/`leiaute_dps_producao` da tabela `configuracao` a cada emissão (`Banco::leiaute_dps`) e `validar_pedido` faz o mesmo para a prévia. Padrão: NT 009 na Produção Restrita, NT 004 na Produção. UI em Configurações → Leiaute da DPS. Cada leiaute é validado com xmllint no seu pacote em `xsd/` (20261020 e 20260727).

## Tried and ruled out
Uma chave global única: a Produção e a Restrita mudam de leiaute em datas diferentes. Coluna nova em `dps`/migração: a escolha é só configuração.

## Watch out
Diferenças NT 004 × NT 009: `finNFSe` (IBSCBS × infDPS), ordem `cNBS`/`xDescServ`, `CST`/`cClassTrib` (em `gIBSCBS` × direto em `trib`). Novo campo que mude de lugar entre leiautes precisa de ramo em `gerar_dps` e de caso no teste `valida_no_xsd_oficial`. O PDF lê os dois lugares.
