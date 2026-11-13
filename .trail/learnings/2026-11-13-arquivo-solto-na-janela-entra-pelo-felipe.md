---
title: "Arquivo solto na janela entra pelo evento do Tauri, não pela interface"
author: Felipe
date: 2026-11-13
branch: feat/arrastar-arquivo-na-planilha
cost: { minutes: 3, tokens: 165465 }
touches:
  - crates/app/src/comandos/dialogo.rs
  - crates/app/src/main.rs
  - docs/specs/0002-arrastar-arquivo-na-planilha/plan.md
  - docs/specs/0002-arrastar-arquivo-na-planilha/tasks.md
---

## Decided
- Spec 0002: o arrastar usa `WindowEvent::DragDrop` no `on_window_event` de `crates/app/src/main.rs`, que chama `comandos::dialogo::ao_soltar`. O backend escolhe (`arquivo_solto`), registra na mesma `Estado::arquivos_escolhidos` do diálogo e emite `arquivo-solto` com só `{ id, nome }` ou `{ erro }`.
- Nenhum comando novo: a interface lê o conteúdo por `reler_arquivo_escolhido(id)`, que só aceita ids registrados.
- A interface decide se o evento vale (tela Importar planilha visível, sem emissão nem leitura em curso).

## Tried and ruled out
- `drop` do HTML / comando recebendo caminho: abriria a leitura de qualquer arquivo pela interface e não funciona igual no WebKitGTK.

## Watch out
- A lista de extensões existe em Rust (`EXTENSOES_SOLTAR`) e no JS (`FILTRO_PLANILHA_EMAIL`); mudar uma exige mudar a outra.
- O Outlook clássico entrega arquivo virtual: nenhum caminho chega no evento.
