---
title: cargo run no Windows falha com perl not found
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 1, tokens: 21843 }
touches:
  - Cargo.lock
  - docs/desenvolvimento.md
---

## Decided
O erro "Command 'perl' not found" no build vem do openssl-src: o libsqlite3-sys (nfse-db) usa bundled-sqlcipher-vendored-openssl e compila o OpenSSL, cujo Configure precisa de Perl. Solução: instalar Strawberry Perl (winget install StrawberryPerl.StrawberryPerl) e reabrir o terminal. Já documentado em docs/desenvolvimento.md.

## Tried and ruled out
Perl do Git (C:\Program Files\Git\usr\bin\perl.exe, MSYS): existe, mas não está no PATH do PowerShell e costuma falhar no Configure do OpenSSL no Windows. Não testado, não recomendado.

## Watch out
Depois de instalar o Perl, reabrir o terminal para o PATH atualizar. A primeira compilação demora alguns minutos.
