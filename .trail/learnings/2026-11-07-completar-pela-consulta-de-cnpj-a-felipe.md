---
title: Completar pela consulta de CNPJ a planilha de tomadores que só tem CNPJs
author: Felipe
date: 2026-11-07
branch: main
cost: { minutes: 1, tokens: 1901 }
id: l-2c458e68
scope: repo
status: active
src: agent
touches:
  - crates/nfse-emissor/src/planilha_tomadores.rs
  - crates/app/src/comandos/tomadores.rs
  - crates/app/src/cnpj.rs
  - crates/app/ui/js/tomadores.js
---

## Decided
- Lista só de CNPJs (ou CNPJ e nome) na importação de tomadores: o leitor marca a linha `a_completar` (CNPJ válido, não repetido, sem nome ou sem nenhuma parte do endereço) e a tela chama `completar_tomador_importado` uma linha por vez, porque assim há andamento sem evento Tauri, a consulta para ao fechar o diálogo e não passa do limite da BrasilAPI (um comando de lote faria tudo isso mais difícil).
- A linha completada é validada de novo pelo mesmo `linha()` (as células vão e voltam na `LinhaTomador`), porque uma segunda regra de validação divergiria da primeira.
- Só células vazias recebem dado da consulta, porque o que veio na planilha é do escritório; o endereço da Receita só entra inteiro e só na linha sem nenhuma parte de endereço, senão misturaria CEP de um lugar com rua de outro.
- CNPJ já cadastrado (qualquer empresa) usa o nome gravado e não consulta, porque o endereço vazio mantém o gravado na importação; consultar trocaria um cadastro conferido pelo da Receita.

## Watch out
- HTTP 429 da BrasilAPI: `consultar_em_lote` espera 5 s, 10 s e 15 s e tenta de novo, porque numa lista longa o limite aparece; a consulta do formulário continua sem espera.
- No Bash tool, heredoc com `$$` (JS) foi barrado pela checagem de segurança de rm; grave o script com a ferramenta Write.

In the code: crates/nfse-emissor/src/planilha_tomadores.rs (LinhaTomador::completar, cnpj_a_completar), crates/app/src/comandos/tomadores.rs, crates/app/src/cnpj.rs, crates/app/ui/js/tomadores.js.
