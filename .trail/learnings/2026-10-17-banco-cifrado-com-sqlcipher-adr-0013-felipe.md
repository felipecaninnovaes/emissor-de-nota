---
title: Banco cifrado com SQLCipher (ADR 0013)
author: Felipe
date: 2026-10-17
branch: feat/sqlcipher
cost: { minutes: 81, tokens: 229588 }
touches:
  - AGENTS.md
  - Cargo.lock
  - README.md
  - crates/app/src/comandos/aparencia.rs
  - crates/app/src/comandos/backup.rs
  - crates/app/src/comandos/cofre.rs
  - crates/app/src/comandos/comum.rs
  - crates/app/src/comandos/email.rs
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/comandos/empresas.rs
---

## Decided
Banco inteiro cifrado com SQLCipher; chave = SHA-256(rótulo + chave Argon2 do cofre), chave AES dos certificados inalterada (migração de banco antigo sem reimportar). Cabeçalho do cofre vai para `emissor.cofre` ao lado do `emissor.db`; aparência vai para `aparencia.json`. `Estado.banco` é `None` até a senha; Bloquear fecha o banco. Banco em claro antigo é convertido na primeira abertura (`sqlcipher_export`), original vira `emissor.db.legado`.

## Tried and ruled out
`resolver = "1"` para unificar features: descartado, o workspace usa resolver 3. `&mut SqliteConnection` e `migrate!` dentro de comando Tauri: o rustc não prova `Send` ("not general enough"); resolvido rodando a abertura em `spawn_blocking` + `block_on` (`em_thread_bloqueante`). Pool de 1 conexão no lugar de `&mut conn` não bastou sozinho.

## Watch out
No Windows o build de host dos macros do sqlx compila `libsqlite3-sys` sem `bundled*`; a build-dependency em `nfse-db` (com o `build.rs` existente, que NÃO pode ser sobrescrito) corrige. Copiar `emissor.db` e `emissor.cofre` juntos. Cuidado ao gerar arquivos com heredoc: conferir se já existem (`git status`) antes de sobrescrever.
