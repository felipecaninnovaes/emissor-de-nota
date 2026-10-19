---
title: "Seletores de arquivo nativos (issue #8)"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 3, tokens: 71558 }
touches:
  - crates/app/src/comandos/dialogo.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/componentes.js
  - crates/app/ui/js/configuracoes.js
  - crates/app/ui/js/empresas.js
  - crates/app/ui/js/formularios.js
  - crates/app/ui/js/planilha.js
  - crates/app/ui/js/servicos.js
  - docs/desenvolvimento.md
---

## Decided
- A interface não usa mais `<input type="file">`: no Linux o WebKitGTK abre o diálogo do GTK, fora do xdg-desktop-portal. Campos de arquivo são input readonly com `data-arquivo` + botão "Escolher arquivo…", ligados por `ligarEscolhaArquivo` (componentes.js).
- Comandos em `comandos/dialogo.rs`: `escolher_arquivo` (só caminho, usado no .nfbk), `ler_arquivo_escolhido` (nome + bytes; certificado e planilhas, cujos comandos seguem recebendo bytes) e `salvar_arquivo` (destino da restauração; nome sugerido à parte para não misturar separadores no Windows).
- `ler_arquivo_escolhido` não recebe caminho da tela: só lê o que o operador escolheu no diálogo, para não virar leitura arbitrária de arquivo.
## Tried and ruled out
- Nome `abrir_arquivo`: já existe em `comandos/notas.rs` e o macro do Tauri colide (E0428) mesmo em módulos diferentes.
## Watch out
- Campo de arquivo obrigatório: a validação usa `data-arquivo` para a mensagem "Escolha o arquivo."; o conteúdo fica numa variável do módulo e o diálogo precisa zerá-la no reset.
