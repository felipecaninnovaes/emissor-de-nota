---
title: "NBS embutida e campos de código com busca (issue #9)"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 7, tokens: 90629 }
touches:
  - crates/app/src/comandos/servicos.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/formularios.js
  - crates/app/ui/js/servicos.js
  - crates/nfse-core/examples/gerar_lista_servicos.rs
  - crates/nfse-core/src/lib.rs
  - crates/nfse-core/src/lista_servicos.rs
  - docs/arquitetura.md
  - docs/desenvolvimento.md
---

## Decided
- NBS 2.0 vem da aba `LISTA.NBS_v2.0` do mesmo Anexo B; o exemplo gerar_lista_servicos grava lista-servicos-nacional.tsv e nbs.tsv numa rodada. Leitura/busca comuns em nfse_core::tabela_codigos (ItemCodigo; ItemListaServico e ItemNbs são aliases).
- Campos #servico-codigo-trib-nac e #servico-nbs viraram combobox (ligarCampoDeCodigo em servicos.js); o campo separado de busca saiu.
- Formatos ctn/nbs usam codigoDeLista: a máscara só tira pontuação quando o texto não tem letras (soDigitos apagaria o nome digitado ao sair do campo); a validação exige só dígitos.

## Tried and ruled out
- Correlação cTribNac ↔ NBS: o Anexo B (v1.01 de 22/01/2026) não traz; não há o que sugerir.

## Watch out
- Na aba NBS os códigos vêm em formatos mistos ("10101", "1.0101.1", "1.0101.11.00 " com espaço); só os de 9 dígitos vão no cNBS. Posição sem subdivisões já vem como X.XXXX.00.00 e não tem nível acima (grupo vazio, 45 casos). O fim da aba tem linhas de modelo (99999, 9.9999.99.99) sem descrição: ignorar.
- Teste de teclado no jsdom: KeyboardEvent precisa de cancelable: true, senão o preventDefault do combobox não vale e o atalho global de Enter (formularios.js) parece enviar o formulário.
