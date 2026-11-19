# 0003 — Cancelamento em lote com aviso por e-mail

- **Status:** implementada <!-- rascunho | aprovada | implementada | abandonada -->
- **Autor:** Felipe CN
- **Data:** 2026-11-19
- **ADRs relacionados:** —

## Contexto e problema
Quando várias notas de uma empresa precisam ser canceladas pelo mesmo motivo (por exemplo, emitidas em duplicidade ou com o mesmo erro), hoje o operador abre o diálogo de cancelamento de cada nota, digita a mesma justificativa e, depois, reenvia uma por uma pela ação "E-mail" para avisar o tomador. O trabalho é repetitivo e fácil de errar.

O e-mail de uma nota cancelada também não diz por que ela foi cancelada: ele reaproveita o texto da emissão ("Segue em anexo a NFS-e…") e só acrescenta "CANCELADA -" no assunto e um aviso no começo do corpo.

## Objetivos
- Cancelar de uma vez várias notas autorizadas da lista de "Notas emitidas", todas com o mesmo motivo e a mesma justificativa.
- Avisar o tomador de cada nota cancelada por e-mail, com um texto próprio de cancelamento, que o escritório pode editar.
- Usar o mesmo e-mail de cancelamento no cancelamento de uma nota só e no reenvio de uma nota cancelada.

## Fora de escopo
- Cancelar notas de várias empresas de uma vez: o lote vale para a empresa selecionada (a lista de "Notas emitidas" já é por empresa).
- Motivo ou justificativa diferentes por nota dentro do mesmo lote.
- Cancelamento por substituição (evento 105102) e por análise fiscal (105104).
- Anexar o XML do evento de cancelamento ao e-mail.
- Conferir o prazo de cancelamento do município: continua sendo da Sefin.
- Cancelar notas de forma automática ou agendada.

## Requisitos
| ID | Requisito | Prioridade |
|---|---|---|
| R1 | Em "Notas emitidas", cada nota `autorizada` com NFS-e tem uma caixa de marcação. Há também "marcar todas", que vale para as autorizadas da lista exibida. Notas em outra situação não podem ser marcadas. | obrigatório |
| R2 | Com ao menos uma nota marcada, aparece a ação "Cancelar selecionadas (N)". Ela abre um diálogo com as notas marcadas (número, tomador e valor), a quantidade e o valor total, um único motivo e uma única justificativa (mesmas regras do cancelamento individual: 15 a 255 caracteres, sem quebra de linha, só os caracteres aceitos pelo `TSString`), e um botão de confirmação com o texto "Cancelar N notas". | obrigatório |
| R3 | O lote cancela as notas uma por uma, pelo mesmo cancelamento de uma nota só (`Emissor::cancelar`), mostrando o andamento ("Cancelando i de N…"). A falha de uma nota não interrompe as outras. | obrigatório |
| R4 | No fim do lote, um resumo separa as notas canceladas, as recusadas pela Sefin (com a mensagem da Sefin) e as sem confirmação (em que repetir o cancelamento é seguro). Ao fechar o resumo, a lista é recarregada. | obrigatório |
| R5 | O diálogo do lote e o do cancelamento individual têm a caixa "Avisar o tomador por e-mail", marcada por padrão. Marcada, cada nota cancelada com sucesso recebe o e-mail de cancelamento logo depois. Sem e-mail configurado em Configurações, a caixa fica desmarcada e desabilitada, com uma dica dizendo onde configurar. | obrigatório |
| R6 | Em Configurações > E-mail há assunto e corpo próprios do e-mail de cancelamento, com texto padrão. Esse texto aceita as variáveis do e-mail da emissão e mais `{motivo}` (nome do motivo) e `{justificativa}`. | obrigatório |
| R7 | Todo e-mail de nota cancelada (no cancelamento individual, no lote ou no reenvio pela ação "E-mail") usa o modelo de cancelamento, no lugar do texto da emissão com o prefixo "CANCELADA -". Os anexos continuam os de hoje: o PDF com a marca d'água de cancelada e o XML da NFS-e. | obrigatório |
| R8 | `{motivo}` e `{justificativa}` vêm do evento de cancelamento gravado no banco. Isso vale também para a nota cancelada no portal nacional e registrada por "Conferir cancelamentos". | obrigatório |
| R9 | Uma falha no envio do e-mail (SMTP, tomador sem e-mail) não desfaz o cancelamento nem interrompe o lote. O resumo informa as notas canceladas sem e-mail enviado e o motivo, e o reenvio continua disponível pela ação "E-mail". | obrigatório |
| R10 | Cada envio é registrado como hoje (`envio_email`, auditoria `email_enviado`) e aparece em "E-mail enviado em…" na lista. | obrigatório |

## Critérios de aceite
Cada critério vira pelo menos um teste.

- **CA1 (R3)**: Dado três notas autorizadas e uma Sefin simulada que aceita as três, quando o lote é cancelado com o motivo "Erro na emissão" e uma justificativa válida, então as três ficam `cancelada`, cada uma com o seu evento gravado, e os três pedidos enviados à Sefin têm o mesmo `cMotivo` e o mesmo `xMotivo`.
- **CA2 (R3, R4)**: Dado três notas e uma Sefin que recusa a segunda (sem evento na consulta), quando o lote roda, então a primeira e a terceira ficam `cancelada`, a segunda continua `autorizada` e o resultado da segunda traz a mensagem de rejeição.
- **CA3 (R3, R4)**: Dado uma nota em que a Sefin não responde e a consulta do evento também falha, quando o lote roda, então a nota continua `autorizada` e o resultado a classifica como "sem confirmação". Dado que a consulta encontra o evento, então a nota fica `cancelada`, como no cancelamento individual.
- **CA4 (R2)**: Dado uma justificativa com 14 caracteres, com quebra de linha ou com um caractere fora do `TSString`, quando se tenta confirmar o lote, então nenhuma nota é enviada à Sefin e o erro aparece junto do campo.
- **CA5 (R1)**: Dado uma lista com notas autorizadas, canceladas, rejeitadas e incertas, quando se usa "marcar todas", então só as autorizadas ficam marcadas.
- **CA6 (R5, R6, R8)**: Dado o e-mail configurado e "Avisar o tomador" marcado, quando uma nota é cancelada com o motivo "Serviço não prestado" e a justificativa J, então um e-mail é entregue aos destinatários do tomador, com o assunto e o corpo do modelo de cancelamento, em que `{motivo}` vira "Serviço não prestado" e `{justificativa}` vira J.
- **CA7 (R7)**: Dado uma nota já cancelada, quando ela é reenviada pela ação "E-mail", então a mensagem usa o modelo de cancelamento (sem o texto da emissão), com o PDF marcado como cancelado e o XML anexados.
- **CA8 (R8)**: Dado uma nota cancelada no portal e registrada por `sincronizar_cancelamento`, quando o e-mail dela é enviado, então `{motivo}` e `{justificativa}` vêm preenchidos a partir do evento consultado.
- **CA9 (R9)**: Dado um lote de duas notas em que o SMTP falha na primeira, quando o lote roda, então as duas ficam `cancelada`, a segunda recebe o e-mail e o resultado da primeira diz "cancelada, e-mail não enviado" com o erro. A falha fica registrada em `envio_email`.
- **CA10 (R9)**: Dado um tomador sem e-mail cadastrado, quando a nota é cancelada com "Avisar o tomador" marcado, então ela fica `cancelada` e o resultado diz que o e-mail não foi enviado por falta de destinatário.
- **CA11 (R5)**: Dado "Avisar o tomador" desmarcado, quando o lote roda, então nenhum e-mail é enviado.
- **CA12 (R6)**: Dado uma configuração de e-mail gravada antes desta versão (sem o modelo de cancelamento), quando ela é lida, então o assunto e o corpo de cancelamento vêm com o texto padrão, sem erro.

## Casos de borda e erros
- **Nada marcado:** "Cancelar selecionadas" não aparece.
- **Lista recarregada no meio (troca de mês ou de empresa):** as marcações são limpas. O lote em andamento usa a lista de notas confirmada no diálogo.
- **Nota cancelada por outro caminho entre a marcação e o lote** (portal, outra janela): `cancelar` recusa a nota por não estar `autorizada` ou registra o evento que encontra na Sefin. A nota entra no resumo pelo que de fato aconteceu.
- **Atualização do sistema em andamento:** o lote não começa (a mesma guarda do cancelamento individual, `atualizando`). Enquanto o lote roda, a atualização é adiada, como já acontece com `cancelamentoEmCurso`.
- **Evento gravado ilegível ou sem `xMotivo`:** `{motivo}` e `{justificativa}` ficam vazios e o e-mail sai assim mesmo.
- **Variáveis desconhecidas no modelo:** o tratamento é o mesmo do modelo da emissão.
- **Repetir o lote com as notas que ficaram "sem confirmação":** é seguro. O Id do pedido de evento é fixo por chave, então não duplica.
- **Ambiente:** cada nota é cancelada no ambiente gravado na sua DPS, como hoje.

## Invariantes (o que não pode mudar)
- O cancelamento de cada nota continua passando por `Emissor::cancelar`, com a mesma consulta do evento em caso de recusa ou falta de resposta e o mesmo registro (`registrar_cancelamento`). Não há outra rota de cancelamento.
- O XML do pedido de cancelamento (e101101) não muda.
- Uma falha no e-mail nunca desfaz nem bloqueia um cancelamento.
- O e-mail da emissão (modelo atual, `enviar_ao_emitir`) não muda para notas autorizadas.
- Uma configuração SMTP já gravada continua sendo lida sem migração (os campos novos têm valor padrão).
- O agente não cancela notas em Produção. Os testes reais são feitos pelo usuário.

## Restrições
- **Sefin:** as notas são canceladas uma de cada vez, sem paralelismo, para não sobrecarregar a Sefin nem o certificado, e para o andamento ser exato.
- **Privacidade:** a justificativa e os e-mails não vão para o log. O registro de envio continua como hoje.
- **Interface:** sem bundler e sem estilo em linha. Os erros de validação aparecem junto do campo, e a interface chama o backend por `chamar`.
- **Validação:** a justificativa é conferida no backend antes do primeiro envio do lote (mesmas regras do `TSString`), não só em JS.

## Questões em aberto
- [x] Limite de notas por lote: a lista já mostra no máximo 500. Vale um limite menor, ou um aviso acima de certo número? **Resolvido:** sem limite extra (vale o teto de 500 da lista); acima de 20 notas o diálogo mostra um aviso a mais.

## Histórico de mudanças
| Data | Mudança | Motivo |
|---|---|---|
| 2026-11-19 | Rascunho | Pedido de cancelamento em lote com aviso por e-mail |
| 2026-11-19 | Aprovada | Revisão do usuário |
| 2026-11-19 | Implementada | Questão do limite fechada (sem limite extra, aviso acima de 20 notas); tarefas T1 a T6 concluídas |
