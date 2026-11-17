---
title: Busca de município por nome no local da prestação
author: Felipe
date: 2026-11-17
branch: feat/0001-importacao-de-pedidos-por-email
cost: { minutes: 1, tokens: 48950 }
---

## Decided
- Ainda em proposta, aguardando o usuário: embutir a tabela IBGE de municípios (código, nome, UF) como `crates/nfse-core/dados/municipios.tsv`, lida pela `Tabela` de `tabela_codigos.rs` (mesmo padrão da lista de serviços e da NBS), e trocar `#local-prestacao` por uma caixa de busca (`ligarCombobox`) por nome ou código.
- A DPS continua recebendo só o código IBGE em `cLocPrestacao`; a validação fica no `Emissor`.

## Tried and ruled out
- A busca "Não sei o CEP" (ViaCEP) não resolve: ela exige a rua, e o operador muitas vezes só sabe a cidade.
- Ainda não há tabela de municípios no repositório; o IBGE só chega pelo CEP ou pela consulta de CNPJ.

## Watch out
- Os campos IBGE `#tomador-municipio` e `#empresa-municipio` têm o mesmo problema; o escopo (só o local ou os três) depende da resposta do usuário.
- Refazer, copiar e substituir preenchem só o código em `#local-prestacao` (notas.js:197); falta mostrar o nome da cidade.
