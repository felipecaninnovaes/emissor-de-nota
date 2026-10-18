---
title: Busca na Lista de Serviços Nacional no cadastro de serviço
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 4, tokens: 72479 }
touches:
  - ".gitattributes"
  - Cargo.lock
  - crates/app/src/comandos/servicos.rs
  - crates/app/src/main.rs
  - crates/app/ui/app.js
  - crates/app/ui/index.html
  - crates/nfse-core/Cargo.toml
  - crates/nfse-core/src/lib.rs
  - docs/arquitetura.md
  - docs/desenvolvimento.md
---

## Decided
Lista cTribNac embutida em crates/nfse-core/dados/lista-servicos-nacional.tsv (338 códigos do Anexo B v1.01 de 2026-01-22), lida por nfse_core::lista_servicos (buscar/por_codigo/fonte). Gerada pelo exemplo `cargo run -p nfse-core --example gerar_lista_servicos -- <anexo_b...xlsx>` (calamine como dev-dependency); a 1ª linha `# fonte:` guarda o nome do arquivo e serve de versão. Comando Tauri buscar_lista_servicos sem cofre (não lê dados do usuário). Na tela, o combobox do tomador virou ligarCombobox genérico; ligarBuscaTomador usa ele.
## Tried and ruled out
Bloquear código fora da lista no validar do nfse-db: descartado, a lista pode atrasar em relação à Sefin; só aviso na tela.
## Watch out
O código na coluna A da planilha vem numérico (sem zero à esquerda); o gerador monta o código por item/subitem/desdobro e confere com a coluna A. Linhas com desdobro 0 são o subitem da LC 116 e não entram. NBS (aba LISTA.NBS_v2.0 e Anexo VIII de correlação) ainda não usada.
