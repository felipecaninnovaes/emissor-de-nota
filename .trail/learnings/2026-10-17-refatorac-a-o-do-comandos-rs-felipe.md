---
title: "Refatoração do comandos.rs, multiplataforma e release 0.2.4"
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 88, tokens: 319912 }
touches:
  - docs/DIAGNOSTICO-SISTEMA.md
  - crates/app/assets/logo-nfse.svg
  - crates/app/src/comandos/cofre.rs
  - Cargo.lock
  - crates/app/Cargo.toml
  - "\"Escopo \\342\\200\\224 Emissor de NFS-e Multiempresa (Rust).md\""
  - crates/app/assets/logo-nfse-assinatura-horizontal.svg
  - crates/app/assets/logo-nfse.png
  - crates/nfse-pdf/src/testes.rs
  - crates/nfse-pdf/src/fontes.rs
---

## Decided
- `comandos.rs` foi dividido em `crates/app/src/comandos/` (um módulo por assunto; `comum.rs` guarda `R`, `texto`, `exigir_cofre`, a macro `emissor!`, `abrir_no_sistema`, `gravar_e_abrir_csv`). O `generate_handler!` do `main.rs` usa o caminho completo (`comandos::modulo::nome`): um `pub use` não funciona com `#[tauri::command]`. Ao mexer em comandos, compare a lista de nomes `#[tauri::command]` antes e depois: esquecer um no handler compila e quebra a tela.
- Todo comando que lê ou grava dados precisa de `exigir_cofre(&estado)` ou da macro `emissor!`. `ler_planilha` estava sem e foi corrigido.
- `abrir_no_sistema` escolhe `explorer` (Windows), `open` (macOS) ou `xdg-open` (outros). A pasta padrão cai para `HOME` quando não há `USERPROFILE`.
- Fontes do DANFSe (`nfse-pdf/src/fontes.rs`): Windows, Liberation (Linux) e Arial do macOS em `/System/Library/Fonts/Supplemental`; os nomes do macOS não foram confirmados em um Mac.
- Logo oficial da NFS-e: PNG embutido em `crates/app/assets/logo-nfse.png`, gerado de `logo-nfse.svg` (cópia ajustada: fonte Liberation Sans e largura 240). A logo da empresa no DANFSe ficou fora do escopo.
- Copiar nota: `Emissor::dados_para_copiar` (autorizada ou cancelada), comando `dados_para_copiar`, botão "Copiar"; a descrição vem do modelo do serviço, não do texto antigo.
- A versão nos XML vem do app via `Emissor::com_versao`, não do crate `nfse-emissor`.
- Tag `vX.Y.Z` precisa bater com `crates/app/Cargo.toml` (e `Cargo.lock`), senão o workflow Instalador falha no passo de conferência. A `v0.2.3` falhou assim antes de subir a versão.
- Próxima migração do banco é a `0006` (a `0005` é o índice de `dps`); migrações em CRLF.

## Tried and ruled out
- `SqliteSynchronous::Normal`: mantido `Full` por durabilidade da numeração e do estado "enviando".
- Rejeitar nota por assinatura da Sefin inválida (item 1.4) e N+1 em `importacao.rs` (item 2.1): descartados, ver o outro learning desta sessão.

## Watch out
- No shell do usuário, comando com `cd` trava: um hook do zsh roda `eza` e fica parado. Use caminhos absolutos, `git -C` e `cargo --manifest-path`; evite heredocs com Python (use Write e rode o arquivo).
- A pasta `crates/graft/` (cache de ferramenta, sem Cargo.toml) quebra o workspace `crates/*`; o `Cargo.toml` raiz agora tem `exclude = ["crates/graft"]`.
- `cargo test --workspace` falha no Linux em `nfse-nacional::identidade_mtls_a_partir_do_pfx` ("builder error"); é do ambiente, o CI roda no Windows.
- Não há CI para macOS; problemas de caminho, fonte e abertura de pasta só aparecem no uso.
- Banco migrado do Windows guarda a pasta de saída com caminho do Windows; no Linux e no macOS é preciso trocar em Configurações (senão o app cria um diretório com esse nome dentro do projeto).
