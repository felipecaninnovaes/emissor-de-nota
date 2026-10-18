---
title: "Divisão do app.js em módulos ES (issue #10, item 4)"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 19, tokens: 161703 }
touches:
  - crates/app/ui/app.js
  - docs/arquitetura.md
  - docs/decisoes/0001-interface-sem-bundler.md
  - docs/desenvolvimento.md
  - crates/app/ui/js/atualizacao.js
  - crates/app/ui/js/componentes.js
  - crates/app/ui/js/configuracoes.js
  - crates/app/ui/js/consultas.js
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/empresas.js
---

## Decided
- app.js virou entrada + crates/app/ui/js/*.js (um por tela/assunto). Camadas: util (sem imports) ← formularios, componentes, tabela ← telas. navegacao importa as telas (irPara chama carregar*), então há ciclos tela↔navegacao/emitir: aceitos, porque nenhum código de carga lê const/let de outro módulo em ciclo; só chama function declarations (hoisted) que usam util.
- Divisão feita por script: reorganizar seções dentro do arquivo único, analisar dependências com acorn + eslint-scope (nomes cruzados, escritas cruzadas, leituras na carga) e gerar import/export a partir disso.

## Tried and ruled out
- Registro de telas na navegação (para evitar ciclos): desnecessário; mais invasivo.

## Watch out
- Import é somente leitura: `aparencia` era reatribuída no início → virou carregarAparencia() em preferencias.js. Qualquer `let` exportada só muda no próprio módulo.
- Validar mudanças de módulo: carregar index.html no jsdom com window.__TAURI__ falso e importar app.js no Node (cópia da pasta com package.json "type":"module"); comparar o DOM final com o da versão anterior. Pegou zero diferenças nesta divisão.
