# 0004 — Enviar o e-mail das notas selecionadas

- **Status:** implementada <!-- rascunho | aprovada | implementada | abandonada -->
- **Autor:** Felipe CN
- **Data:** 2026-11-19
- **ADRs relacionados:** —

## Contexto e problema
O e-mail da nota só sai sozinho na emissão quando "Enviar automaticamente" está marcado em Configurações > E-mail. Quando a caixa está desmarcada, ou o envio falhou, o operador precisa reenviar nota a nota pela ação "E-mail" da linha, o que é repetitivo quando várias notas do mês ficaram sem e-mail.

A spec 0003 criou a seleção por caixa de marcação em "Notas emitidas" para o cancelamento em lote. Esta spec usa a mesma seleção para enviar o e-mail de várias notas de uma vez. Ela muda o R1 e o CA5 da 0003: a caixa passa a valer também para notas canceladas, e o cancelamento em lote filtra as autorizadas entre as marcadas.

## Objetivos
- Enviar de uma vez o e-mail de várias notas marcadas em "Notas emitidas", cada uma aos e-mails cadastrados do seu tomador.
- Mostrar antes para quem vai cada e-mail e quais já foram enviados, e no fim um resumo do que saiu e do que não saiu.

## Fora de escopo
- Mudar o envio automático na emissão (`enviar_ao_emitir`) ou o texto dos modelos de e-mail.
- Trocar ou acrescentar destinatários no lote (cópia, outro endereço): para isso continua a ação "E-mail" da linha.
- Juntar várias notas numa mensagem só (por tomador, por exemplo): é uma mensagem por nota, como hoje.
- Enviar e-mail de notas de várias empresas de uma vez: a lista já é por empresa.
- Envio agendado ou automático fora da emissão.
- Pular automaticamente as notas já enviadas.

## Requisitos
| ID | Requisito | Prioridade |
|---|---|---|
| R1 | Em "Notas emitidas", a caixa de marcação fica habilitada em toda nota com NFS-e que esteja `autorizada` ou `cancelada`. "Marcar todas" marca essas notas da lista exibida. As demais (rejeitada, incerta, enviando, sem NFS-e) continuam sem poder ser marcadas. | obrigatório |
| R2 | Ao lado de "Cancelar selecionadas (N)" há o botão "Enviar e-mail (N)". Cada botão conta e age só sobre as notas marcadas que servem para ele: o cancelamento, as `autorizadas`; o e-mail, as autorizadas e as canceladas. Cada botão fica escondido quando o seu N é zero. | obrigatório |
| R3 | Sem e-mail configurado em Configurações, "Enviar e-mail (N)" aparece desabilitado, com uma dica dizendo onde configurar (como a caixa "Avisar o tomador"). | obrigatório |
| R4 | O botão abre um diálogo com as notas (número, tomador, situação), para quem vai cada e-mail (os e-mails cadastrados do tomador) e, se já houve envio, "enviado em …". Uma nota sem destinatário aparece como "sem e-mail cadastrado". O diálogo mostra quantas notas vão e, se houver, quantas já tinham sido enviadas. O botão de confirmação diz "Enviar N e-mails" (N = notas com destinatário). | obrigatório |
| R5 | Confirmado, o envio é nota a nota, pelo mesmo envio da ação "E-mail" (`Emissor::enviar_email` com os destinatários do cadastro), com o andamento ("Enviando i de N…"). A falha de uma nota não interrompe as outras. Nota cancelada sai com o modelo de cancelamento, como no envio individual. | obrigatório |
| R6 | Notas sem destinatário não são enviadas; vão direto para o resumo como não enviadas, com o motivo. | obrigatório |
| R7 | No fim, um resumo separa as notas enviadas (com os destinatários) e as não enviadas (com o motivo: falha do SMTP, sem destinatário etc.). Ao fechar, a lista é recarregada. | obrigatório |
| R8 | Cada envio é registrado como hoje (`envio_email`, auditoria `email_enviado`) e aparece em "E-mail enviado em…" na lista. | obrigatório |

## Critérios de aceite
Cada critério vira pelo menos um teste.

- **CA1 (R5, R8)**: Dado três notas com destinatário e um carteiro simulado que aceita tudo, quando o lote envia as três, então cada uma recebe uma mensagem só para os e-mails do seu tomador e as três passam a ter "E-mail enviado em".
- **CA2 (R5, R7)**: Dado três notas e um carteiro que falha na segunda, quando o lote roda, então a primeira e a terceira são enviadas, a segunda fica sem "E-mail enviado em" e o resultado dela traz o erro do SMTP.
- **CA3 (R5)**: Dado uma nota cancelada no lote, quando o e-mail é enviado, então a mensagem usa o modelo de cancelamento, com o PDF marcado como cancelado.
- **CA4 (R4, R6)**: Dado uma nota cujo tomador não tem e-mail cadastrado, quando o diálogo abre, então ela aparece como "sem e-mail cadastrado", não entra na contagem do botão de confirmação, nenhuma mensagem é enviada para ela e o resumo a mostra como não enviada por falta de destinatário.
- **CA5 (R1, R2)**: Dado uma lista com notas autorizadas, canceladas, rejeitadas e incertas, quando se usa "marcar todas", então ficam marcadas as autorizadas e as canceladas; "Enviar e-mail (N)" conta todas elas e "Cancelar selecionadas (N)" conta só as autorizadas.
- **CA6 (R2)**: Dado só notas canceladas marcadas, então "Cancelar selecionadas" fica escondido e "Enviar e-mail (N)" aparece; o cancelamento em lote nunca recebe uma nota cancelada.
- **CA7 (R3)**: Dado o e-mail não configurado, quando há notas marcadas, então "Enviar e-mail (N)" aparece desabilitado, com a dica.
- **CA8 (R4)**: Dado notas marcadas em que duas já têm "E-mail enviado em", quando o diálogo abre, então ele mostra a data do envio de cada uma e o aviso "2 já tinham sido enviadas"; confirmado, as duas são enviadas de novo.
- **CA9 (R4)**: Dado os destinatários das notas marcadas, quando o diálogo abre, então os e-mails mostrados são os mesmos para os quais o envio vai (os do cadastro do tomador, ou o e-mail do cadastro quando não há e-mails de envio).

## Casos de borda e erros
- **Nada marcado (ou nenhuma marcada que sirva):** o botão correspondente não aparece.
- **Lista recarregada (troca de mês ou de empresa):** as marcações são limpas, como hoje. O lote em andamento usa a lista confirmada no diálogo.
- **Cadastro do tomador alterado entre o diálogo e o envio:** o envio usa os destinatários lidos na hora do envio de cada nota (como `enviar_email` com `para` vazio).
- **SMTP fora do ar:** todas falham, cada uma com o erro; nada é interrompido no meio e o resumo lista todas como não enviadas.
- **Configuração de e-mail apagada com o diálogo aberto:** cada envio falha com "configure o envio de e-mail" e entra no resumo.
- **Cancelamento em lote em andamento:** o envio de e-mails não começa, e vice-versa (um lote de cada vez na tela).
- **Repetir o lote:** envia de novo; não há idempotência, cada envio é uma mensagem nova (como o reenvio individual).

## Invariantes (o que não pode mudar)
- O envio de cada nota passa por `Emissor::enviar_email`; não há outro caminho de montagem ou entrega de e-mail.
- O cancelamento em lote (spec 0003) continua só com notas `autorizadas`, mesmo com canceladas marcadas.
- A ação "E-mail" da linha, o envio automático na emissão e os modelos de e-mail não mudam.
- O registro dos envios (`envio_email`, auditoria) é o mesmo de hoje.

## Restrições
- **SMTP:** um envio de cada vez, sem paralelismo, para não esbarrar em limites do provedor e para o andamento ser exato.
- **Privacidade:** os endereços e o conteúdo das mensagens não vão para o log.
- **Interface:** sem bundler e sem estilo em linha; chamadas por `chamar`.

## Questões em aberto
- [x] Limite de notas por envio: vale o teto de 500 da lista, com aviso a mais acima de 20 notas, como no cancelamento em lote? **Resolvido:** sim, teto de 500 da lista e aviso a mais no diálogo acima de 20 notas.

## Histórico de mudanças
| Data | Mudança | Motivo |
|---|---|---|
| 2026-11-19 | Rascunho | Pedido de envio do e-mail de várias notas de uma vez |
| 2026-11-19 | Aprovada | Revisão do usuário; limite: teto de 500 da lista, aviso acima de 20 notas |
| 2026-11-19 | Implementada | Tarefas T1 a T4 concluídas; teste no app real com o usuário |
