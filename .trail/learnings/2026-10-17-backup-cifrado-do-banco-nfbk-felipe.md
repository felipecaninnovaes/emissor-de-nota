---
title: Backup cifrado do banco (.nfbk)
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 5, tokens: 69889 }
touches:
  - crates/app/src/comandos/cofre.rs
  - crates/app/src/comandos/mod.rs
  - crates/app/src/main.rs
  - crates/app/ui/app.js
  - crates/app/ui/index.html
  - crates/nfse-db/src/lib.rs
  - docs/arquitetura.md
  - docs/decisoes/0002-sqlite-por-maquina.md
  - docs/deploy.md
  - docs/escopo.md
---

## Decided
Backup = só o banco (`VACUUM INTO` em temp do sistema), cifrado em blocos de 1 MiB com a chave do cofre; cabeçalho com sal/params do Argon2, então a senha mestra abre em outra máquina. O banco já guarda os XMLs (tabelas nfse/evento/dps), então não se copia a pasta de saída. Automático ao abrir o cofre (20 h), mantém 10 cópias. SQLCipher descartado: certificados já cifrados; resto via criptografia de disco.

## Tried and ruled out
Cifrar com senha própria do backup: exigiria a senha a cada backup automático. `block_in_place` no teste: panica no runtime single-thread do `#[tokio::test]`.

## Watch out
Nome do temp precisa de contador atômico (testes em paralelo colidiam). `trail check` acusa falso conflito com "pasta" nos docs/UI; ignorar. Restauração na UI só gera um banco novo; trocar o `emissor.db` é manual.
