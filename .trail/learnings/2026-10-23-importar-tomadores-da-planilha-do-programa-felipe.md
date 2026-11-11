---
title: Importar tomadores da planilha do programa antigo (clientes.xlsx)
author: Felipe
date: 2026-10-23
branch: main
cost: { minutes: 3, tokens: 62569 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/tomadores.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/componentes.js
  - crates/app/ui/js/tomadores.js
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/lib.rs
  - docs/usuario.md
  - crates/nfse-emissor/src/planilha_tomadores.rs
---

## Decided
- A planilha do programa antigo (`clientes.xlsx`) tem documento, nome, e-mails, cidade/UF, rua, número e bairro, mas **não tem CEP nem código IBGE**. Como `planilha_tomadores.rs` recusa endereço começado sem CEP, município, logradouro e bairro, o usuário preferiu importar **sem endereço** e completar no app.
- Só entram os tomadores ativos, que são os CNPJ/CPF do CSV de importação de notas. Para cruzar, compare só os dígitos sem zeros à esquerda (o CSV de notas perde os zeros).
- E-mails: "E-mail" fica com o primeiro e-mail do cliente (pulando os do escritório) e "E-mails de envio" com a lista inteira do programa antigo, separada por "; ".
- A coluna IE do programa antigo é a inscrição estadual, não a municipal: não vai para "Inscricao municipal".

## Tried and ruled out
- openpyxl e pandas não estão instalados no Python da máquina: o xlsx foi lido com zipfile + xml.etree (sharedStrings + sheet1.xml).
- Buscar CEP na BrasilAPI ou no ViaCEP foi oferecido e recusado: isso envia dados de clientes para um serviço externo.

## Watch out
- O console do Windows mostra os acentos do xlsx corrompidos; use PYTHONIOENCODING=utf-8. O CSV gerado é UTF-8 sem BOM, que o leitor aceita.
- Tomadores com "municipio" no nome podem ser entes públicos (tipo Municipal, com tpEnteGov): confirme com o usuário em vez de marcar como Privado sem perguntar.
