---
title: Importar tomadores por planilha e retenção herdada no vínculo
author: Felipe
date: 2026-10-23
branch: main
cost: { minutes: 18, tokens: 228760 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/tomadores.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/tomadores.js
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/lib.rs
  - docs/usuario.md
  - crates/nfse-emissor/src/planilha_tomadores.rs
  - crates/app/ui/js/componentes.js
---

## Decided
- `planilha_tomadores` (nfse-emissor) lê CSV/XLSX/XLS/ODS como a de serviços; obrigatórios só CNPJ/CPF e Nome. Município só pelo código IBGE (7 dígitos); CPF/CNPJ/CEP recebem os zeros à esquerda que a planilha tira. Linha com erro volta com `item: None` e o texto do documento e nome para a tela.
- `Banco::importar_tomadores` grava tudo numa transação e trata `None` como "manter o gravado": IM, endereço, telefone e e-mail da pessoa e e-mails/retenção/tipo do vínculo (COALESCE). Só importa na empresa escolhida, sem a escolha de várias empresas da importação de serviços.
- Vincular tomador de outra empresa copia retenção e tipo (`retencao_de_outra_empresa`: o vínculo com algo marcado; entre eles, o mais recente por rowid). O vínculo novo da importação herda igual quando a planilha não traz as colunas.
- IM obrigatória é da empresa emissora (só `required` no formulário), não do tomador; a importação de tomadores aceita tomador sem IM (decisão do usuário).

## Tried and ruled out
- Pegar a retenção do vínculo mais recente sem prioridade: uma empresa que vinculou sem marcar nada apagaria a retenção herdada.
- CPF zerado como lugar vago do documento na linha com erro: `Cpf::parse` recusa dígitos todos iguais.

## Watch out
- No shell do Bash tool, heredoc com aspas simples dentro de `python - <<'EOF'` às vezes quebra; grave o script num arquivo do scratchpad. Arquivos em `crates/app/src` estão em CRLF no disco.
- Três alterações nos mesmos arquivos (CHANGELOG, docs/usuario.md): guardar patch/cópia de cada etapa e montar o índice com `git apply --cached` / `git update-index --cacheinfo` para um commit por alteração.
