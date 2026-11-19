# 0003 — Tarefas

Cada tarefa: um commit, `just check` verde ao terminar, um teste que a prova.

- [x] **T1** — Justificativa e motivo do evento no `nfse-xml`
  - Requisitos: R2, R8
  - Arquivos: `crates/nfse-xml/src/evento.rs`, `crates/nfse-xml/src/lib.rs`
  - Teste: `evento::testes::justificativa_*` (CA4: 14 caracteres, quebra de linha, caractere fora do `TSString`), `evento::testes::motivo_do_cancelamento_*` (evento correto, sem `xMotivo`, ilegível); testes XSD do pedido continuam verdes
  - Verificar: `just test -p nfse-xml`

- [x] **T2** — Modelo de cancelamento no `ConfigSmtp` e evento nos dados do e-mail
  - Requisitos: R6, R8
  - Arquivos: `crates/nfse-db/src/emails.rs`, `crates/app/src/comandos/email.rs` (`ConfigSmtp::padrao()`)
  - Teste: `emails::testes::config_antiga_le_modelo_de_cancelamento_padrao` (CA12), `emails::testes::dados_email_traz_evento_de_cancelamento`
  - Verificar: `just test -p nfse-db emails`

- [x] **T3** — E-mail de nota cancelada usa o modelo próprio
  - Requisitos: R6, R7, R8
  - Arquivos: `crates/nfse-emissor/src/email.rs`, `crates/nfse-emissor/src/envio_email.rs`, `crates/nfse-emissor/src/testes.rs`
  - Teste: `email::testes::cancelada_usa_modelo_de_cancelamento` (CA7), `testes::reenvio_de_nota_cancelada_usa_modelo_de_cancelamento` (CA7), `testes::email_de_nota_cancelada_no_portal_traz_motivo` (CA8)
  - Verificar: `just test -p nfse-emissor email`

- [x] **T4** — `CancelamentoIncerto` e `cancelar_e_avisar`
  - Requisitos: R3, R5, R9, R10
  - Arquivos: `crates/nfse-emissor/src/lib.rs`, `crates/nfse-emissor/src/cancelamento.rs`, `crates/nfse-emissor/src/testes.rs`
  - Teste: CA1 (três notas, mesmo `cMotivo`/`xMotivo`), CA2 (recusa da segunda), CA3 (sem resposta → `CancelamentoIncerto`; consulta acha o evento → cancelada), CA6 (e-mail com motivo e justificativa), CA9 (SMTP falha na primeira, segunda recebe), CA10 (tomador sem e-mail), CA11 (`avisar = false`)
  - Verificar: `just test -p nfse-emissor cancel`

- [ ] **T5** — Comandos `cancelar` (com aviso e resultado classificado) e `conferir_justificativa_cancelamento`
  - Requisitos: R2, R4, R5
  - Arquivos: `crates/app/src/comandos/notas.rs`, `crates/app/src/main.rs`
  - Teste: `comandos::notas::testes::classifica_resultado_do_cancelamento` (cancelada, recusada com a mensagem da Sefin, sem confirmação, e-mail não enviado), `comandos::notas::testes::confere_justificativa` (CA4)
  - Verificar: `just test -p emissor-nfse notas`

- [ ] **T6** — Interface: seleção, diálogo do lote, aviso no individual e modelo em Configurações; CHANGELOG e docs
  - Requisitos: R1, R2, R3, R4, R5, R6, R7
  - Arquivos: `crates/app/ui/index.html`, `crates/app/ui/js/notas.js`, `crates/app/ui/js/configuracoes.js`, `CHANGELOG.md`, `docs/usuario.md`, `docs/fiscal.md`
  - Teste: manual no app (`just run`; cancelamento real só pelo usuário):
    - CA5: lista com notas autorizadas, canceladas, rejeitadas e incertas; "marcar todas" marca só as autorizadas
    - R2: "Cancelar selecionadas (N)" só aparece com marcação; o diálogo mostra lista, quantidade, total e o botão "Cancelar N notas"; acima de 20 notas, aparece o aviso a mais
    - CA4: justificativa com 14 caracteres, com quebra de linha ou com travessão → erro junto do campo e nada enviado
    - R3, R4: andamento "Cancelando i de N…" e resumo em três grupos; fechar recarrega a lista
    - R5: sem e-mail configurado, "Avisar o tomador" fica desmarcada e desabilitada, com a dica
    - R6, R7: modelo editável em Configurações; reenvio de nota cancelada sai com o modelo de cancelamento
    - Troca de mês ou de empresa limpa as marcações
  - Verificar: `just check`

- [ ] **T7** — Fechamento da spec
  - Requisitos: —
  - Arquivos: `docs/specs/0003-cancelamento-em-lote/spec.md` (status `implementada`, questão em aberto fechada: sem limite extra, com aviso acima de 20 notas; histórico); `docs/arquitetura.md` só se couber
  - Verificar: `just check`

## Notas de desvio
<!-- T#: o que mudou em relação ao plano e por quê -->
- T3: `envio_email.rs` não precisou mudar (a troca do modelo é toda em `email::montar`). A linha do CHANGELOG sobre o e-mail de nota cancelada entrou aqui, no commit que muda o comportamento, e não na T6.
- T4: no CA9, o registro da falha em `envio_email` é conferido pelo efeito visível (a lista não mostra "E-mail enviado em" para a nota cuja entrega falhou), já que o `nfse-db` não tem leitura dos envios com erro; a gravação é a mesma de `enviar_email`. O SMTP que falha só na primeira nota é simulado com dois carteiros, um por chamada.
