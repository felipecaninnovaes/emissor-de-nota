---
title: "Tomador baixado: a Sefin rejeita? (não confirmado)"
author: Felipe
date: 2026-11-06
branch: main
cost: { minutes: 1, tokens: 8367 }
touches:
  - crates/app/ui/js/consultas.js
---

## Decided
O usuário perguntou se a API da Sefin rejeita uma DPS cujo tomador tem CNPJ baixado. A validação no app é ele quem faz. No app, hoje só há um aviso no cadastro do tomador quando a BrasilAPI devolve uma situação diferente de "Ativa" (crates/app/ui/js/consultas.js, ligarConsultaCnpj). A emissão não confere a situação.

## Tried and ruled out
Busca na web: só blogs. Um deles cita E0190 ("CNPJ do tomador não encontrado no cadastro CNPJ") e diz que tomador baixado cai aí, mas não é fonte oficial. Não há fonte oficial confirmando.

## Watch out
Para ter certeza: conferir a planilha de regras de negócio/erros do Anexo I (DPS) da NFS-e Nacional, ou testar na Produção Restrita (só com pedido explícito; ela aceita apenas contribuintes cadastrados, E0084). Não afirmar o código de rejeição sem essa conferência.
