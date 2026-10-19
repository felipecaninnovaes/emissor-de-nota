---
title: "cIndOp não é calculado, é escolhido por serviço"
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 10, tokens: 19627 }
touches:
  - Cargo.lock
---

## Decided
O cIndOp (indicador da operação, 6 dígitos) não é calculado em lugar nenhum: o usuário escolhe no select de Serviços (index.html, #servico-ibs-ind-op), salva no cadastro e o valor segue direto para o XML (nfse-xml/src/dps.rs, campo cIndOp). No PDF (nfse-pdf/src/dados.rs, indicador_local) ele é exibido junto de cLocalidadeIncid/xLocalidadeIncid da NFS-e e da UF derivada do IBGE.

## Tried and ruled out
Procurar a lógica de cálculo: não existe. A validação em nfse-core/src/modelo.rs só confere 6 dígitos, sem checar contra a tabela do Anexo VIII nem contra cTribNac/cClassTrib.

## Watch out
servicos.js adiciona ao select qualquer indicador salvo que não esteja nas opções, então um valor antigo ou errado reaparece sem aviso. Os códigos dos testes (100301, 030101) são só dados de teste.
