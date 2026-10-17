---
title: "Erro 'migration 1 was previously applied but has been modified' ao copiar banco"
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 16, tokens: 22836 }
---

## Decided
O banco (app_data_dir/emissor.db; no Linux ~/.local/share/br.emissor-nfse.app/) guarda checksums sqlx das migrações calculados sobre os bytes CRLF (build Windows). Checkout com LF dá esse erro. Correção: apagar crates/nfse-db/migrations/*.sql e rodar `git checkout -- crates/nfse-db/migrations` para o .gitattributes (eol=crlf) aplicar. Se persistir, touch nos .sql ou cargo clean -p nfse-db (sqlx::migrate! embute os arquivos na compilação).

## Tried and ruled out
Não era mudança de conteúdo: git status limpo e os checksums do banco batem com `sed 's/$/\r/' | sha384sum` de cada migração.

## Watch out
`rm` está aliasado como interativo no shell do usuário; use `command rm -f`. Checkouts anteriores ao commit 488a7af (.gitattributes) ficam com LF.
