# 0004 — Plano técnico

- **Spec:** [spec.md](spec.md)
- **Status:** aprovado

## Abordagem
**O lote roda na tela, com uma chamada do comando `enviar_email` por nota**, como o cancelamento em lote da 0003 (`crates/app/ui/js/notas.js`, `cancelarEmSequencia`). O comando já existe e já envia a nota cancelada com o modelo de cancelamento; com `para` vazio, `Emissor::enviar_email` (`crates/nfse-emissor/src/envio_email.rs:8`) usa os destinatários do cadastro do tomador e registra o envio (`envio_email`, auditoria). Não há rota nova de envio.

Questão da spec fechada: teto de 500 da lista e aviso a mais no diálogo acima de 20 notas.

### Backend
- **`nfse-db` (`emails.rs`):** a parte de `dados_email_nfse` que acha os destinatários (e-mails de envio do vínculo `empresa_tomador`, senão o e-mail do cadastro, senão o da DPS) sai para `pub async fn destinatarios_nfse(nfse_id) -> Resultado<Vec<String>>`, sem ler o XML da nota. `dados_email_nfse` passa a chamá-la, então o diálogo mostra exatamente para quem o envio vai (CA9).
- **App (`comandos/email.rs`):** comando novo `destinatarios_notas(nfse_ids: Vec<i64>) -> Vec<DestinatariosDto { nfse_id, para }>`, com `exigir_cofre`, registrado no `generate_handler!` com o caminho completo. Uma chamada para todas as notas marcadas, em vez de uma por nota ao abrir o diálogo. `destinatarios_email` (diálogo individual) passa a usar `destinatarios_nfse`.
- `nfse-emissor` não muda; ganha testes que fixam o comportamento do lote (CA1, CA2, CA4).

### Interface
- **`tabela.js` (`selecaoEmTabela`):** `botao` e `rotulo` passam a ser opcionais. Sem eles, a função só cuida das caixas, de "marcar todas" e de `marcados()`. Os outros cinco usos não mudam.
- **`notas.js`:**
  - `podeMarcar(n)` = `nfse_id` e situação `autorizada` ou `cancelada` (R1); `podeCancelar` continua só `autorizada`.
  - `atualizarSelecao()` conta os dois botões pelas marcadas filtradas: "Cancelar selecionadas (N)" com `podeCancelar`, "Enviar e-mail (N)" com todas as marcadas (R2, CA5, CA6). Cada um some com N = 0. O de e-mail fica desabilitado sem e-mail configurado, com o texto de ajuda ao lado (R3, CA7); `configurado` vem de `obter_email` ao carregar a lista. Ambos ficam desabilitados com um lote em andamento.
  - `cancelarSelecionadas` recebe `marcados().filter(podeCancelar)`.
  - O laço de `cancelarEmSequencia` vira `emSequencia(notas, { form, andamento, botao, verbo }, uma)`, usado pelos dois lotes (trava o diálogo, andamento "Cancelando/Enviando i de N…", falha de uma não para as outras). `cancelamentoEmCurso` (que a atualização consulta) passa a valer para os dois lotes; o nome fica, com comentário.
  - O resumo vira genérico: `#dlg-resumo-cancelamento` passa a `#dlg-resumo-lote`, com título definido por quem abre; cada lote monta os próprios grupos.
- **Diálogo `#dlg-email-lote` (R4, R6, CA4, CA8):** tabela com número, tomador, situação, para (ou "sem e-mail cadastrado") e "enviado em"; aviso com quantas já tinham sido enviadas e, acima de 20, o aviso a mais; botão "Enviar N e-mails" com N = notas com destinatário (desabilitado com N = 0). Ao confirmar: guarda `atualizando`, depois `emSequencia` só com as notas com destinatário, chamando `enviar_email` com `para: ""` (o backend relê os destinatários na hora). As sem destinatário entram direto no resultado como "sem e-mail cadastrado".
- **Resumo (R7):** "N enviada(s)" com os destinatários e "N não enviada(s)" com o motivo (erro do SMTP, sem destinatário). Fechar recarrega a lista.

## Alternativas descartadas
| Alternativa | Por que não |
|---|---|
| Comando único `enviar_emails_lote` no backend | Sem andamento nota a nota; a 0003 já decidiu pelo laço na tela. |
| Chamar `destinatarios_email` uma vez por nota ao abrir o diálogo | Até 500 chamadas, cada uma lendo o XML da nota (`dados_email_nfse`). Um comando com a lista e sem XML é mais leve. |
| Passar ao `enviar_email` os destinatários mostrados no diálogo | Se o cadastro mudar entre o diálogo e o envio, mandaria para o endereço antigo; com `para` vazio vale o cadastro da hora, como no envio automático. |
| Duas instâncias de `selecaoEmTabela` na mesma tabela | Duplicaria os ouvintes de "marcar todas" e as contagens brigariam pelo mesmo botão. |
| Esconder o botão sem e-mail configurado | O usuário escolheu desabilitado com dica (spec, R3). |

## Mudanças
| Módulo / arquivo | Mudança | Requisitos |
|---|---|---|
| `crates/nfse-db/src/emails.rs` | `destinatarios_nfse`; `dados_email_nfse` usa ela | R4, R5 |
| `crates/app/src/comandos/email.rs`, `main.rs` | comando `destinatarios_notas`; `destinatarios_email` usa `destinatarios_nfse` | R4 |
| `crates/nfse-emissor/src/testes.rs` | testes do envio em sequência | R5, R6, R8 |
| `crates/app/ui/js/tabela.js` | `botao`/`rotulo` opcionais | R2 |
| `crates/app/ui/js/notas.js`, `index.html`, `estilo.css` (se preciso) | `emSequencia`, resumo genérico, dois botões, diálogo do e-mail em lote | R1–R7 |
| `CHANGELOG.md`, `docs/usuario.md` | linha em `## Não publicado`; uso na tela | — |
| `docs/specs/0003-cancelamento-em-lote/spec.md` | linha no histórico: R1/CA5 alterados pela 0004 | — |

## Contratos alterados
- Comando novo `destinatarios_notas(nfseIds)`. Os demais comandos não mudam.
- `nfse-db`: função pública nova `destinatarios_nfse`; `DadosEmailNfse` não muda.
- Sem migração, sem mudança no XML nem nos modelos de e-mail.

## Estratégia de teste
| Requisito / CA | Tipo de teste | Onde |
|---|---|---|
| R4 / CA9 | integração (banco em memória) | `nfse_db::emails::testes`: `destinatarios_nfse` igual a `dados_email_nfse().destinatarios` com e-mails de envio, só com o cadastro e sem e-mail |
| R5, R8 / CA1 | integração (Sefin e carteiro simulados) | `nfse_emissor::testes`: três notas enviadas em sequência, cada mensagem só para o seu tomador, as três com `email_enviado_em` |
| R5, R7 / CA2 | integração | `nfse_emissor::testes`: carteiro que falha na segunda; a segunda sem `email_enviado_em` e com o erro |
| R5 / CA3 | integração (existente) | `testes::reenvio_de_nota_cancelada_usa_modelo_de_cancelamento` |
| R6 / CA4 | integração | `nfse_emissor::testes`: tomador sem e-mail → `destinatarios_nfse` vazio e nada entregue |
| R1, R2, R3, R4, R7 / CA5–CA8 | manual (`just run`, ou a cópia da interface com o Tauri simulado, como na 0003) | roteiro da T4 em [tasks.md](tasks.md) |

## Riscos e mitigação
- **Refatorar o laço e o resumo do cancelamento pode quebrar o lote da 0003:** a T3 é só a refatoração, sem comportamento novo, conferida no roteiro do cancelamento em lote antes da feature.
- **Caixa de nota cancelada chegar ao cancelamento em lote:** `cancelarSelecionadas` filtra por `podeCancelar`; mesmo assim, `Emissor::cancelar` recusa nota fora de `autorizada`.
- **Limite do provedor de SMTP num lote grande:** envio sequencial; a falha vira o resultado da nota e o resumo mostra o motivo.

## Rollback
Reverter os commits. Não há dado novo gravado nem migração.

## Decisões que viram ADR
- Nenhuma: segue os padrões da 0003.
