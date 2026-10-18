---
title: Lista de serviços nacional (cTribNac) não vem por API
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 1, tokens: 25913 }
touches:
  - crates/nfse-db/src/servicos.rs
---

## Decided
Nada implementado ainda. Pesquisa: a API do Sistema Nacional não tem endpoint que liste serviços. A Lista de Serviços Nacional (cTribNac de 6 dígitos + descrição + correlação NBS) só existe como planilha anexa à documentação técnica (gov.br/nfse). Proposta feita ao usuário: embutir a lista como dado estático, buscar por nome só no cadastro de serviço da empresa (crates/nfse-db/src/servicos.rs) e manter a emissão só com os serviços cadastrados.

## Tried and ruled out
API de Parâmetros Municipais (/parametros_municipais/...) como fonte da lista: ela só consulta a partir de município + código já conhecido (alíquota, regimes, deduções) ou por CPF/CNPJ (retenções, benefícios). Serve para validar o código escolhido, não para descobrir um código pelo nome.

## Watch out
Os manuais do contribuinte e do município (v1.2) dão caminhos diferentes para os endpoints de parâmetros; confira a versão vigente antes de integrar. A lista nacional muda com a documentação (reforma tributária), então registre a versão da planilha e deixe a tabela fácil de regenerar.
