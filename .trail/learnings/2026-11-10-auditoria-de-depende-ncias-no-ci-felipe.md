---
title: Auditoria de dependências no CI (cargo deny advisories)
author: Felipe
date: 2026-11-10
branch: main
cost: { minutes: 9, tokens: 84765 }
touches:
  - ".github/workflows/ci.yml"
  - ".github/workflows/instalador.yml"
  - ".github/workflows/nuvem.yml"
  - Cargo.lock
  - crates/nfse-core/Cargo.toml
  - crates/nfse-emissor/Cargo.toml
  - deny.toml
  - ".github/dependabot.yml"
  - tmp-n\303\243o-commitar/dps.xml
  - tmp-n\303\243o-commitar/emitida-pelo-portal-nacional.xml
---

## Decided
- CI roda `cargo deny check advisories licenses` (workspace e servidor) em push, PR e toda segunda (cron). `deny.toml` tem `[advisories]` com `unmaintained = "workspace"` (transitivas de Tauri/GTK/zbus não barram).
- Exceções com motivo no `deny.toml`: RUSTSEC-2023-0071 (rsa, só assina local), RUSTSEC-2026-0459 (SQLite -wal; sqlx 0.8 fixa libsqlite3-sys 0.30 e o SQLCipher corrigido não saiu), 0367/0368/0369/0429 (zip 4 do tauri-plugin-updater, pacote assinado antes de extrair), 0192 (ttf-parser, também usado pelo krilla; troca pelo skrifa pendente).
- Ações dos workflows fixadas por SHA com a tag em comentário; Dependabot mensal para github-actions. `tauri-cli@2.12.4` exato no instalador (mesma do devbox.json).
- calamine 0.38 (traz zip 9); dev-dependency `zip` do nfse-emissor subiu de 7 para 9.

## Tried and ruled out
- Subir libsqlite3-sys para 0.39: impossível sem sqlx 0.9, e o bundled-sqlcipher 0.39 continua afetado.

## Watch out
- Pelo Bash tool (Git Bash) o build do openssl-sys falha: o perl do Git vem antes do Strawberry. Compilar pelo PowerShell.
- `.claude/settings.json` é CRLF: replace em Python com "\n" não casa; use o Edit.
- Ao tirar uma exceção do deny.toml, rode também no servidor: lá ela vira só aviso "advisory-not-detected".
