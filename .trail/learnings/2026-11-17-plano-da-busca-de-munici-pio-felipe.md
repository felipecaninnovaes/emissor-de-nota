---
title: Plano da busca de município por nome (spec 0002)
author: Felipe
date: 2026-11-17
branch: feat/0002-busca-de-municipio-por-nome
cost: { minutes: 9, tokens: 75592 }
touches:
  - docs/specs/0002-busca-de-municipio-por-nome/plan.md
  - docs/specs/0002-busca-de-municipio-por-nome/tasks.md
  - crates/nfse-pdf/src/dados.rs
---

## Decided
- Tabela de municípios embutida em crates/nfse-core/dados/municipios.tsv (código, nome, UF), gerada da DTB 2025 do IBGE por um exemplo novo gerar_municipios.rs; módulo próprio nfse_core::municipios com tipo Municipio.
- uf_do_ibge desce de crates/nfse-pdf/src/dados.rs para nfse_core::municipios (utilitário no crate mais baixo).
- UI: ligarCampoDeCodigo (servicos.js) vira componente compartilhado em componentes.js; formato `ibge` passa a codigoDeLista(7) para aceitar texto e dar erro se o nome não for escolhido.

## Tried and ruled out
- Reusar tabela_codigos::Tabela: ordena pela posição na tabela e põe "contém" no mesmo nível de "igual" ("sao paulo" traria São Paulo das Missões/RS antes de São Paulo); a busca de município ordena por UF pedida > nome igual > começa com > contém.

## Watch out
- Vários caminhos põem o código no campo sem digitação (CEP, consulta CNPJ, usar endereço do tomador, refazer/copiar/substituir, abrir cadastro); todos precisam chamar o descrever devolvido, senão o nome abaixo fica velho.
- Com o campo aceitando texto, um nome não escolhido viraria vazio em localPrestacao() e a nota sairia no município da empresa; a validação de formato no Revisar impede isso.
- tmp-não-commitar/ é bloqueada para leitura: a DTB precisa estar em outro caminho.
