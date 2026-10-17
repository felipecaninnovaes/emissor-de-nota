---
title: Tomador é global (pessoa); só o vínculo é por empresa
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 1, tokens: 28643 }
touches:
  - docs/fiscal.md
---

## Decided
Validado no código: o cadastro do tomador já é compartilhado. `pessoa` tem `documento` único e `salvar_tomador` faz upsert por documento (teste `tomador_reutilizado_entre_empresas`). `empresa_tomador` é só o vínculo e guarda e-mails, observação, `retem_federais` e `retem_iss`. A lista por empresa vem do JOIN no vínculo (`buscar_tomadores`, `tomador()`), não de cadastro duplicado.
O vínculo deve continuar por empresa: a retenção depende de empresa × tomador (ADR 0012, docs/fiscal.md), e e-mails e observação são de cada empresa. Nenhuma regra da Sefin obriga a separação; é escolha do projeto.

## Tried and ruled out
Tornar retenção, e-mails e observação globais: a retenção de uma empresa mudaria as notas de outra (risco fiscal).

## Watch out
Melhoria possível, ainda não feita: a busca sugerir pessoas já cadastradas em outras empresas, com botão "vincular" (hoje `pessoa_por_documento` só evita nova consulta de CNPJ).
