---
title: Item 1.4 descartado e demais decisões da Fase 1 e 2
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 41, tokens: 93438 }
touches:
  - docs/DIAGNOSTICO-SISTEMA.md
  - crates/nfse-emissor/src/lib.rs
  - "\"Escopo \\342\\200\\224 Emissor de NFS-e Multiempresa (Rust).md\""
  - crates/app/src/comandos.rs
  - crates/nfse-db/migrations/0005_indice_dps_empresa.sql
  - crates/nfse-emissor/Cargo.toml
  - crates/nfse-db/src/lib.rs
  - Escopo — Emissor de NFS-e Multiempresa (Rust).md
---

## Decided
- Item 1.4 do DIAGNOSTICO-SISTEMA descartado: manter a verificação da assinatura da Sefin (`nfse_xml::verificar`, nfse-emissor/src/lib.rs `gravar`) como aviso, sem abortar a gravação.
- PDF (DANFSe) e assinatura XMLDSIG rodam em `spawn_blocking`; a versão nos XML vem do app via `Emissor::com_versao`.
- Mantido `SqliteSynchronous::Full` (durabilidade da numeração e do estado "enviando"); o índice de dps foi na migração nova 0005, em CRLF.

## Tried and ruled out
- Abortar a gravação quando a assinatura da Sefin não confere: a nota já foi autorizada e registrada em `registrar_autorizacao`; abortar deixaria a nota válida sem XML/PDF e o sistema divergente da Sefin. A resposta já chega autenticada por mTLS.
- Item 2.1 (N+1 em importacao.rs): empresas e serviços já são cacheados; sobram ~4 consultas locais por linha, sem ganho perceptível.

## Watch out
- Validar a cadeia ICP-Brasil da Sefin exigiria embarcar as ACs raiz; está como pendente no Escopo.md e só valeria como aviso extra.
- Em comandos longos no shell do usuário, evitar `cd` e heredocs: um hook/alias do zsh chama `eza` e pode travar o shell.
