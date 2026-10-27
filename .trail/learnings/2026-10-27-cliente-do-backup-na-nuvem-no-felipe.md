---
title: Cliente do backup na nuvem no app
author: Felipe
date: 2026-10-27
branch: main
cost: { minutes: 10, tokens: 141987 }
touches:
  - CHANGELOG.md
  - crates/app/Cargo.toml
  - crates/app/src/comandos/backup.rs
  - crates/app/src/comandos/mod.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/configuracoes.js
  - crates/nfse-db/src/lib.rs
  - docs/arquitetura.md
  - docs/usuario.md
---

## Decided
- Config em `nfse-db/src/nuvem.rs`: endereço (`configuracao.nuvem`, só https ou http em localhost via `normalizar_endereco`), token cifrado no cofre (contexto `token-nuvem`, hex), `nuvem_ultimo` RFC 3339; `sha256_arquivo` fica no nfse-db (já tem sha2/hex) para o app não ganhar dependência.
- Cliente HTTP em `crates/app/src/nuvem.rs`; comandos em `comandos/nuvem.rs`. Envio consulta `/situacao` antes: assinatura inativa nem gera backup.
- `fazer_backup` poda a pasta, então o envio usa pasta temporária própria (apagada no Drop). O guard do emissor é solto antes do upload para não travar o "Bloquear".
- Restauração reaproveita `backup::preparar_restauracao` + `backup::reiniciar` (extraídas de `restaurar_e_reiniciar`); arquivo baixado com nome nosso (`nuvem-<id>.nfbk`), nunca o nome vindo do servidor.
- Automático: `backup::automatico` roda o local (se houver pasta) e depois `nuvem::automatico` (se houver token), com o mesmo `vencido()` de 20 h; falha só audita `backup_nuvem_falhou`.

## Tried and ruled out
- Guardar o emissor (read lock) durante todo o upload: bloquearia o Bloquear/restaurar por minutos.

## Watch out
- Upload em streaming (`reqwest::Body::from(tokio::fs::File)`, feature `stream`) com `Content-Length` posto à mão: conferir no servidor real que não vai chunked.
- Em heredoc Python, `"\n"` dentro de JS virou quebra de linha real; rode `node --check` no JS depois de editar por script.
