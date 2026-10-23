---
title: Duplicar serviço
author: Felipe
date: 2026-10-23
branch: main
cost: { minutes: 3, tokens: 62493 }
touches:
  - CHANGELOG.md
  - crates/app/ui/js/servicos.js
  - crates/nfse-db/src/servicos.rs
  - docs/usuario.md
---

## Decided
- "Duplicar" em Serviços reaproveita `editarServico(s, { copia: true })` (ui/js/servicos.js): formulário preenchido com o original, nome com " (cópia)", e o `id` vai `null`, então `salvar_servico` cria outro registro. Sem comando Tauri novo nem mudança no banco.
- A tabela `servico` não tem índice único nem validação de nome/código repetido; o teste `copia_com_mesmo_codigo_e_outro_servico` (nfse-db/src/servicos.rs) fixa isso.

## Tried and ruled out
- Comando `duplicar_servico` no backend: desnecessário, a cópia passa pelo salvar normal e o operador ajusta antes de gravar.

## Watch out
- Com 3 ações, `acoesLinha` mostra só a primeira e manda as outras para "Mais ▾" (Remover saiu da vista). O menu é popover dentro da linha, então o clique ainda chega ao listener de `#lista-servicos`.
- A importação de serviços atualiza pelo nome (`chave_nome`); a cópia com "(cópia)" no nome não colide, mas se o operador der o mesmo nome do original, uma importação futura casa só com um deles.
