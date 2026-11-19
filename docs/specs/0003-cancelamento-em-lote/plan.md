# 0003 — Plano técnico

- **Spec:** [spec.md](spec.md)
- **Status:** aprovado

## Abordagem
**O lote roda na tela, com uma chamada por nota.** O JS percorre as notas confirmadas no diálogo e chama o comando `cancelar` de cada uma, como já fazem "Conferir cancelamentos" (`crates/app/ui/js/notas.js:89`) e a emissão em sequência (`crates/app/ui/js/lote.js:154`). Não há rota nova de cancelamento: o backend só ganha o aviso por e-mail depois do cancelamento.

Questão em aberto da spec: **sem limite extra**. Vale o teto de 500 da lista. Acima de 20 notas, o diálogo mostra um aviso a mais.

### Backend
- **`nfse-xml` (`evento.rs`):** a conferência da justificativa sai de `gerar_pedido_cancelamento` para `pub fn conferir_justificativa(&str) -> Result<(), PedidoInvalido>` (15 a 255 caracteres depois do `trim`, mais `conferir_tsstring`), e `gerar_pedido_cancelamento` passa a chamá-la. O XML não muda. Entram também `MotivoCancelamento::nome()` ("Erro na emissão" etc.), `MotivoCancelamento::de_codigo(&str)` e `pub fn motivo_do_cancelamento(xml_evento) -> Option<(Option<MotivoCancelamento>, String)>`, que lê `cMotivo` e `xMotivo` do `e101101` com `roxmltree`, como faz `e_cancelamento_da_nfse`.
- **`nfse-db` (`emails.rs`):** `ConfigSmtp` ganha `assunto_cancelamento` e `corpo_cancelamento` com `#[serde(default = ...)]` e as constantes `ASSUNTO_CANCELAMENTO_PADRAO` e `CORPO_CANCELAMENTO_PADRAO`, para uma configuração antiga ser lida sem erro (CA12). `DadosEmailNfse` ganha `xml_cancelamento: Option<String>`, que `dados_email_nfse` lê do último `evento` com `tipo = 'cancelamento'` da nota. `ConfigSmtp::padrao()` substitui a cópia do valor padrão em `obter_email` (`crates/app/src/comandos/email.rs:24`) e nos testes.
- **`nfse-emissor` (`email.rs`):** `preencher` passa a trocar também `{motivo}` e `{justificativa}`, que ficam vazias sem evento ou com evento ilegível. Para nota cancelada (`dados.cancelada`), `montar` usa `assunto_cancelamento` e `corpo_cancelamento` no lugar do prefixo "CANCELADA -" (R7). Os anexos não mudam.
- **`nfse-emissor` (`cancelamento.rs`):** o caso "a Sefin não confirmou" deixa de ser `ErroEmissao::Regra` e vira a variante nova `ErroEmissao::CancelamentoIncerto(String)`, com o mesmo texto, para o app separar "sem confirmação" de "recusada" sem comparar texto. O método novo `Emissor::cancelar_e_avisar(nfse_id, motivo, justificativa, avisar, carteiro) -> Result<Option<Result<Vec<String>, String>>, ErroEmissao>` chama `cancelar`, que devolve o erro adiante se falhar, e, com `avisar`, chama `enviar_email(nfse_id, &[], carteiro)`. Um erro no e-mail vira `Some(Err(..))` e nunca desfaz o cancelamento (R9). `enviar_email` (`envio_email.rs:8`) já registra o envio em `envio_email` e na auditoria, inclusive no caso "sem destinatário" (R10).

### App
- **`comandos/notas.rs`:** `cancelar` recebe `avisar: bool` e devolve `ResultadoCancelamentoDto { situacao: "cancelada" | "recusada" | "sem_confirmacao", mensagem, email: Option<{ enviado, para, erro }> }`. Uma função pura faz a classificação: `Ok` → cancelada; `CancelamentoIncerto` → sem_confirmacao; os demais erros → recusada, com a mensagem. Isso inclui `Rejeitada` (com a mensagem da Sefin), nota fora de `autorizada` e falha do certificado. O comando novo `conferir_justificativa_cancelamento(justificativa)` não toca dados nem usa o cofre e confere a justificativa no backend antes do primeiro envio (CA4). Os dois entram no `generate_handler!` com o caminho completo.
- **Interface:**
  - `index.html`/`notas.js`: "Notas emitidas" ganha uma coluna de caixas de marcação (colspan 8 → 9), habilitada só para nota `autorizada` com `nfse_id` (R1, CA5), mais "marcar todas" no cabeçalho e o botão "Cancelar selecionadas (N)". A seleção reaproveita `selecaoEmTabela` (`crates/app/ui/js/tabela.js:93`), que já pula as caixas desabilitadas. Recarregar a lista limpa as marcações.
  - Diálogo `#dlg-cancelar-lote`: lista (número, tomador e valor), quantidade, total (`somarValores`), motivo, justificativa (`data-contador`), "Avisar o tomador por e-mail" e o botão "Cancelar N notas". Ao confirmar:
    1. `conferir_justificativa_cancelamento`; se der erro, `ErroCampo` no campo e nada é enviado;
    2. a guarda `atualizando`;
    3. `cancelamentoEmCurso = true` enquanto o laço roda, com o andamento "Cancelando i de N…".

    O resumo aparece no próprio diálogo, em três grupos: canceladas; não canceladas, com a mensagem; e sem confirmação, com o aviso de que repetir é seguro. As notas canceladas sem e-mail enviado aparecem como "cancelada, e-mail não enviado: motivo". Fechar o diálogo recarrega a lista.
  - `#dlg-cancelar` (individual): ganha a mesma caixa "Avisar o tomador" e passa pela mesma validação no backend. O toast diz o que aconteceu com o e-mail.
  - Caixa "Avisar": marcada por padrão. Quando `obter_email().configurado` é falso, ela fica desmarcada e desabilitada, com a dica "Configure o e-mail em Configurações > E-mail" (R5).
  - `configuracoes.js`/`index.html`: assunto e mensagem do cancelamento em Configurações > E-mail; a ajuda das variáveis passa a citar `{motivo}` e `{justificativa}` (R6).
  - `notas.js:264`: no diálogo de reenvio, o texto para nota cancelada passa a dizer que vai o e-mail de cancelamento.

## Alternativas descartadas
| Alternativa | Por que não |
|---|---|
| Comando único `cancelar_lote` no backend | Não daria para mostrar o andamento nota a nota. A tela já faz assim na emissão e na conferência (learning de 2026-11-19). |
| Classificar "sem confirmação" pelo texto do erro no JS | Frágil. A variante `CancelamentoIncerto` dá o tipo sem mudar o texto. |
| `nfse-db` interpretar o XML do evento | O `nfse-db` não lê XML fiscal: devolve o evento, e o `nfse-xml` o interpreta. |
| Gravar motivo e justificativa em colunas novas | Exigiria migração e não cobriria o evento vindo do portal (R8). O evento já guarda os dois. |
| Generalizar `desenharAndamento` do lote de emissão | Os rótulos e as situações são da emissão. No cancelamento basta a linha de andamento no diálogo. |

## Mudanças
| Módulo / arquivo | Mudança | Requisitos |
|---|---|---|
| `crates/nfse-xml/src/evento.rs`, `lib.rs` | `conferir_justificativa`, `nome`/`de_codigo` do motivo, `motivo_do_cancelamento` | R2, R8 |
| `crates/nfse-db/src/emails.rs` | modelo de cancelamento no `ConfigSmtp`; `xml_cancelamento` em `DadosEmailNfse`; `ConfigSmtp::padrao()` | R6, R8 |
| `crates/nfse-emissor/src/email.rs` | `{motivo}`, `{justificativa}`; modelo próprio para nota cancelada | R6, R7, R8 |
| `crates/nfse-emissor/src/lib.rs`, `cancelamento.rs` | `CancelamentoIncerto`; `cancelar_e_avisar` | R3, R5, R9, R10 |
| `crates/app/src/comandos/notas.rs`, `email.rs`, `main.rs` | `cancelar` com aviso e resultado; `conferir_justificativa_cancelamento`; `obter_email` com `padrao()` | R2, R4, R5 |
| `crates/app/ui/index.html`, `js/notas.js`, `js/configuracoes.js` | seleção, diálogo do lote, aviso no individual, modelo em Configurações | R1–R7 |
| `CHANGELOG.md`, `docs/usuario.md`, `docs/fiscal.md` | linha em `## Não publicado`; uso na tela; e-mail do cancelamento | — |

## Contratos alterados
- O comando `cancelar(nfseId, motivo, justificativa, avisar)` passa a devolver `ResultadoCancelamentoDto` (antes, `()`). A única chamadora é `notas.js`. Comando novo: `conferir_justificativa_cancelamento`.
- `ConfigSmtp` (JSON em `configuracao`): dois campos novos com valor padrão, sem migração. `salvar_email` e `obter_email` passam a levar esses campos.
- `ErroEmissao::CancelamentoIncerto`: variante nova, com a mesma mensagem de antes.
- O XML do pedido e101101 não muda. O banco não tem migração.

## Estratégia de teste
| Requisito / CA | Tipo de teste | Onde |
|---|---|---|
| R2 / CA4 | unitário | `nfse_xml::evento::testes` (14 caracteres, quebra de linha, caractere fora do `TSString`) e teste do comando `conferir_justificativa_cancelamento`. Os testes XSD existentes do pedido continuam valendo |
| R8 | unitário | `nfse_xml::evento::testes::motivo_do_cancelamento_*` (evento correto, sem `xMotivo`, ilegível) |
| R6 / CA12 | unitário | `nfse_db::emails::testes` (JSON antigo sem os campos → padrão) e `dados_email_nfse` com `xml_cancelamento` |
| R6, R7 / CA7 | unitário | `nfse_emissor::email::testes` (variáveis novas; nota cancelada usa o modelo de cancelamento, sem o texto da emissão, com PDF e XML) |
| R3 / CA1, CA2, CA3 | integração (Sefin simulada) | `nfse_emissor::testes`: três notas em sequência; mesmo `cMotivo`/`xMotivo` nos três pedidos; recusa da segunda; sem resposta → `CancelamentoIncerto`; consulta que acha o evento → cancelada |
| R5, R8 / CA6, CA8 | integração | `nfse_emissor::testes` com `CarteiroTeste` ("Serviço não prestado" e J no corpo; nota sincronizada do portal, com o motivo do evento) |
| R9, R10 / CA9, CA10, CA11 | integração | `nfse_emissor::testes` (carteiro que falha na primeira nota, tomador sem e-mail, `avisar = false` → nada enviado; envio registrado em `envio_email`) |
| R4 | unitário | classificação pura do resultado em `comandos/notas.rs` |
| R1, R2, R4, R5 / CA5 | manual (`just run`) | roteiro da T6 em [tasks.md](tasks.md) |

## Riscos e mitigação
- Com três notas no mesmo cenário de teste, a Sefin simulada precisa de chaves diferentes: o fixture (`nfse_autorizada`, com número e chave por chamada) é ajustado na T4.
- Um lote longo ocupa o certificado e a Sefin por mais tempo: o lote é sequencial, sem paralelismo (restrição da spec).
- A justificativa não vai para o log: o resumo e a auditoria não a incluem.

## Rollback
Reverter os commits. Uma versão anterior ignora os dois campos novos do `ConfigSmtp` (o serde ignora campos a mais), então a configuração gravada continua sendo lida.

## Decisões que viram ADR
- Nenhuma: segue os padrões existentes.
