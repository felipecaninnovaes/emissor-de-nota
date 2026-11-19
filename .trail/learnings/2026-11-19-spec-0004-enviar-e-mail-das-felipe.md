---
title: "Spec 0004: enviar e-mail das notas selecionadas"
author: Felipe
date: 2026-11-19
branch: feat/email-das-notas-selecionadas
cost: { minutes: 8, tokens: 78631 }
touches:
  - docs/specs/0004-email-das-notas-selecionadas/spec.md
---

## Decided
- A caixa de marcação de "Notas emitidas" vale para autorizadas e canceladas; cada botão ("Cancelar selecionadas (N)", "Enviar e-mail (N)") conta e age só sobre as marcadas que servem para ele. Isso muda o R1/CA5 da 0003.
- Envio nota a nota pela tela, chamando o mesmo envio da ação "E-mail" (`Emissor::enviar_email` com `para` vazio = destinatários do cadastro). Diálogo com destinatários e "enviado em", aviso das já enviadas (envia todas), nota sem destinatário não é enviada e vai ao resumo. Sem SMTP: botão desabilitado com dica. Aviso extra acima de 20 notas, teto 500 da lista.
## Tried and ruled out
- Pular automaticamente notas já enviadas; editar/acrescentar destinatários no lote; juntar notas numa mensagem só.
## Watch out
- `selecaoEmTabela` (tabela.js) recebe um botão só e conta todos os marcados; com dois botões, cada contagem precisa filtrar (podeCancelar vs. pode enviar e-mail).
- O cancelamento em lote não pode receber nota cancelada depois que a caixa passa a aceitá-las.
