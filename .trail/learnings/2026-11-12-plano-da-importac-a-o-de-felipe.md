---
title: Plano da importação de pedidos por e-mail (spec 0001)
author: Felipe
date: 2026-11-12
branch: feat/0001-importacao-de-pedidos-por-email
cost: { minutes: 7, tokens: 55282 }
touches:
  - docs/specs/0001-importacao-de-pedidos-por-email/spec.md
  - docs/specs/0001-importacao-de-pedidos-por-email/plan.md
  - docs/specs/0001-importacao-de-pedidos-por-email/tasks.md
---

## Decided
- E-mail (.eml/.msg/texto colado) entra pelo mesmo `ItemImportado` da planilha: três camadas novas em nfse-emissor (leitura_email, pedido_email, importacao_email); prévia, emissão e idempotência são as da planilha.
- Dependências: mail-parser (MIME, charset, HTML→texto) e cfb (só os streams do corpo do .msg).
- CEP: o app consulta o ViaCEP um por vez e passa um mapa CEP→IBGE ao resolver; o mapa é a consulta simulada nos testes (sem trait).
- Bloco repetido no e-mail vira erro "repetido (já está na linha N)"; a planilha de notas continua com o contador #n.
- Origem continua `Origem::Planilha` (arquivo = nome do e-mail ou "texto colado", linha = nº do bloco): sem migração.
- Só os rótulos da spec; texto corrido aceita CNPJ e CPF.

## Tried and ruled out
- Trait ConsultaCep no núcleo: desnecessária com o mapa pré-resolvido.
- Parser MIME próprio: muito caso de borda de charset.

## Watch out
- Não existe `{mes}`: as variáveis são `{mes_ref}`, `{mes_ref_extenso}`, `{ano_ref}` (montagem.rs).
- A planilha de notas NÃO marca linha repetida (chave com #n); só as de tomadores/serviços marcam.
- A tela Importar planilha não tinha seletor de serviço; `FILTRO_PLANILHA` é compartilhada com outras telas, criar constante nova.
- `gravar_tomador` faz UPSERT da pessoa: não chamar para pessoa existente (o cadastro não pode mudar).
- O cep::consultar vive no crate app, sem trait.
