# 0004 — Tarefas

Cada tarefa: um commit, `just check` verde ao terminar, um teste que a prova.

- [x] **T1** — Destinatários de várias notas (`destinatarios_nfse` e comando `destinatarios_notas`)
  - Requisitos: R4, R5
  - Arquivos: `crates/nfse-db/src/emails.rs`, `crates/app/src/comandos/email.rs`, `crates/app/src/main.rs`
  - Teste: `emails::testes::destinatarios_nfse_*` (CA9: com e-mails de envio do vínculo, só com o e-mail do cadastro, sem e-mail → vazio; sempre igual a `dados_email_nfse().destinatarios`)
  - Verificar: `just test -p nfse-db emails`

- [x] **T2** — Testes do envio em sequência no `nfse-emissor`
  - Requisitos: R5, R6, R8
  - Arquivos: `crates/nfse-emissor/src/testes.rs`
  - Teste: `testes::email_em_sequencia_*` (CA1: três notas, cada uma para o seu tomador, as três com "enviado em"; CA2: carteiro que falha na segunda; CA4: tomador sem e-mail, nada entregue). CA3 já coberto por `reenvio_de_nota_cancelada_usa_modelo_de_cancelamento`
  - Verificar: `just test -p nfse-emissor email`

- [ ] **T3** — Refatoração: laço em sequência e resumo genéricos (sem comportamento novo)
  - Requisitos: — (prepara R5, R7)
  - Arquivos: `crates/app/ui/js/notas.js`, `crates/app/ui/js/tabela.js`, `crates/app/ui/index.html`
  - Teste: manual — roteiro do cancelamento em lote da 0003 continua igual (andamento, resumo nos grupos, fechar recarrega); os outros usos de `selecaoEmTabela` (lote, planilha, importações) continuam contando no botão
  - Verificar: `just check`

- [ ] **T4** — Interface: caixa para canceladas, botão "Enviar e-mail (N)", diálogo e resumo; CHANGELOG e docs
  - Requisitos: R1, R2, R3, R4, R5, R6, R7
  - Arquivos: `crates/app/ui/index.html`, `crates/app/ui/js/notas.js`, `crates/app/ui/estilo.css` (se preciso), `CHANGELOG.md`, `docs/usuario.md`, `docs/specs/0003-cancelamento-em-lote/spec.md` (histórico)
  - Teste: manual no app (`just run`; envio real só pelo usuário):
    - CA5: "marcar todas" marca autorizadas e canceladas; "Enviar e-mail (N)" conta todas, "Cancelar selecionadas (N)" só as autorizadas
    - CA6: só canceladas marcadas → "Cancelar selecionadas" escondido; o diálogo de cancelamento não lista canceladas
    - CA7: sem e-mail configurado → "Enviar e-mail (N)" desabilitado com a dica
    - CA8, CA4: diálogo com para quem vai, "enviado em", aviso das já enviadas, "sem e-mail cadastrado" fora da contagem; acima de 20 notas, o aviso a mais
    - R5, R7: andamento "Enviando i de N…"; resumo enviadas / não enviadas com o motivo; fechar recarrega e mostra "E-mail enviado em"
  - Verificar: `just check`

- [ ] **T5** — Fechamento da spec
  - Requisitos: —
  - Arquivos: `docs/specs/0004-email-das-notas-selecionadas/spec.md` (status `implementada`, histórico), `tasks.md` (notas de desvio)
  - Verificar: `just check`

## Notas de desvio
<!-- T#: o que mudou em relação ao plano e por quê -->
- T2: os testes passaram de primeira, como esperado: fixam o comportamento de `enviar_email` que o lote usa, sem mudar o código de produção. O CA1 usa dois tomadores (dois e-mails de envio) para provar que cada nota vai só ao seu.
