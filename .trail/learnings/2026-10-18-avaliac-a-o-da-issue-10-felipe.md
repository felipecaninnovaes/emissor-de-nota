---
title: "Avaliação da issue #10 (refatoração/modularização)"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 2, tokens: 32831 }
---

## Decided
- Ordem sugerida: normalização de texto única no nfse-core → dividir nfse-emissor/lib.rs em submódulos (impl Emissor em módulos filhos mantém acesso a campos privados) → helpers de UI dentro do app.js → dividir app.js em módulos ES → dps.rs (baixa prioridade).

## Tried and ruled out
- "Unificar leitura de planilhas": já feito; planilha_servicos.rs importa ler_tabela/normalizar de planilha.rs.
- `previa` em nfse-emissor/lib.rs: não existe, apesar de citada na issue.
- normalizar de nfse-core/documento.rs (CPF/CNPJ) não é a mesma coisa; não unificar.

## Watch out
- Há 3 normalizações de acento com semânticas diferentes: lista_servicos.rs (mantém espaços), planilha.rs (só alfanuméricos), importacao.rs::sem_acento_minusculo (tabela incompleta: falta è, ì, ò, ä…).
- refazerNota × copiarNota diferem na descrição (copiar usa o modelo do serviço) e nos avisos; escolherTomador chama descartarRevisao() e guarda o objeto completo, escolherTomadorRecorrencia não.
- Dividir app.js exige um módulo de estado compartilhado (estado, tomadorRecorrencia, loteEmAndamento) e não há testes de UI.
