---
title: "Issue 4: pasta de dados do macOS sem .app (ADR 0014)"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 17, tokens: 65213 }
touches:
  - crates/app/src/main.rs
  - docs/arquitetura.md
  - docs/configuracao.md
  - crates/app/src/pasta_dados.rs
  - docs/decisoes/0014-pasta-de-dados-no-macos.md
---

## Decided
Manter o `identifier` `br.emissor-nfse.app` e, só no macOS, usar a pasta `br.emissor-nfse` (`crates/app/src/pasta_dados.rs`), movendo arquivo a arquivo, sem sobrescrever, o que existia na pasta antiga. O arquivo do `window-state` fica na pasta antiga. Registrado no ADR 0014. Não testado em macOS, só no Linux.

## Tried and ruled out
Mudar o `identifier`: o Tauri deriva o `upgradeCode` do MSI dele, então o Windows deixaria de atualizar no lugar, e o caminho dos dados mudaria em todos os sistemas. Renomear a pasta inteira com um único `rename`: levaria o `window-state` junto e o plugin o recriaria na pasta antiga, perdendo tamanho e posição da janela.

## Watch out
Comandos Bash que começam com `cd` travam neste shell (hook do zsh roda `eza`): usar caminhos absolutos, `git -C` e `--manifest-path`. Imports usados só na migração precisam de `#[cfg(any(target_os = "macos", test))]`, senão o clippy com `-D warnings` falha fora do macOS.
