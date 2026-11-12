# 0001 — Plano técnico

- **Spec:** [spec.md](spec.md)
- **Status:** aprovado

## Abordagem
O e-mail passa por três camadas novas em `nfse-emissor`, puras e testáveis sem rede, e termina no mesmo `ItemImportado` da planilha. Assim, a prévia (`validar_pedido` → `Emissor::validar`), a emissão (`emitir_planilha` → `Emissor::emitir_importado`) e a idempotência (`tentativa_importada`) são as que já existem.

1. **`leitura_email.rs`: bytes → texto.**
   - **`.eml`:** `mail_parser::MessageParser`. Usa `body_text(0)`, que é o text/plain ou, na falta dele, o HTML convertido em texto. O mail-parser decodifica quoted-printable, base64 e charset.
   - **`.msg`:** o crate `cfb` lê só os streams do corpo:
     - `__substg1.0_1000001F` (Unicode);
     - `__substg1.0_1000001E` (ANSI, na página de código de `PR_INTERNET_CPID`/`PR_MESSAGE_CODEPAGE` em `__properties_version1.0`, decodificada com `encoding_rs`, ou Windows-1252 sem declaração);
     - `__substg1.0_10130102` (HTML, convertido pelo conversor do mail-parser).
   - **Mensagens de erro:**
     - corpo só em RTF → "não consegui ler o corpo deste e-mail; cole o texto";
     - contêiner ou MIME inválido → "não consegui abrir o e-mail".
2. **`pedido_email.rs`: texto → `Vec<PedidoEmail>`** (o número da linha é o do bloco).
   - Os rótulos são achados no texto normalizado por `nfse_core::texto::sem_acento_minusculo` (`crates/nfse-core/src/texto.rs:4`).
   - Um tokenizador de rótulos aceita:
     - vários rótulos por linha;
     - valor que continua na linha seguinte até o próximo rótulo;
     - espaços sobrando.
   - **Leiaute de blocos:** um bloco por PACIENTE.
   - **Texto corrido** (sem PACIENTE):
     - o tomador é o primeiro CNPJ/CPF válido (`Cnpj::parse`/`Cpf::parse`, `crates/nfse-core/src/documento.rs:72,140`) que não esteja na lista de CNPJs das empresas cadastradas, recebida por parâmetro;
     - o valor é o primeiro "R$";
     - o período é o primeiro "MM/AAAA".
   - Sem regex: varredura simples do texto.
   - Nenhum leiaute → "nenhum pedido de nota encontrado neste e-mail".
3. **`importacao_email.rs`: `resolver(banco, pedidos, empresa_id, servico_id, mes_padrao, ceps)` → `Vec<ItemImportado>`.**
   - **Conversões:** reusa `valor_da_planilha`, `mes_da_planilha` e `documentos_da_planilha` (`crates/nfse-emissor/src/importacao.rs:71-131`), e a montagem da chave, extraída de `importacao::resolver` (L259-279) numa função comum sem mudar o comportamento.
   - **Serviço:** sem serviço → "escolha o serviço" em todas as linhas.
   - **Descrição:** a do bloco, senão `servico.descricao`, e depois `aplicar_mes_ref` (`montagem.rs:99`).
   - **Tomador cadastrado** (`pessoa_por_documento`, `crates/nfse-db/src/tomadores.rs:523`):
     - o cadastro não muda;
     - um aviso aparece se o nome ou o CEP diferem de `pessoa_cadastrada` (L532);
     - o vínculo com a empresa segue como hoje (`vincular_tomador`).
   - **CPF sem cadastro:** `tomador_novo`, com o município (IBGE) tirado do mapa `ceps`. CEP ausente ou não encontrado → "endereço sem município: confira o CEP".
   - **CNPJ sem cadastro:** o mesmo erro da planilha. O botão **Cadastrar tomador** que já existe atende o R13 (`ui/js/planilha.js:78-81`).
   - **Repetição:** um bloco com a mesma chave-base de um anterior recebe "repetido (já está na linha N)", como em `planilha_tomadores.rs:204-208`. A ocorrência fica sempre `#1`, então reimportar é idempotente.
   - **`ceps_a_consultar(banco, &pedidos)`:** devolve os CEPs dos CPFs sem cadastro, sem repetição.

O mapa de CEP é o ponto de simulação nos testes. No app, ele é preenchido por `cep::consultar` (`crates/app/src/cep.rs:23`), um CEP por vez, e não é preciso trait nova.

### `ItemImportado`
Ganha `avisos: Vec<String>` e `tomador_novo: Option<Tomador>`. Na planilha os dois ficam vazios.

`pedido()` usa `tomador_novo` como tomador da DPS, para a prévia passar em `Emissor::validar`. A T7 confere como o `PedidoEmissao.tomador` é tratado hoje com `vincular_tomador` (`comandos/planilha.rs:54-67`).

### Emissão
`Emissor::emitir_importado` (`lib.rs:696`), antes de `emitir`, no mesmo ponto em que o vínculo é feito hoje (L707-714):
- Se a pessoa ainda não existe: `salvar_tomador`, que grava a pessoa e o vínculo.
- Se já existe (cadastrada entre a leitura e a emissão): usa a pessoa como está e só vincula. O UPSERT de `gravar_tomador` sobrescreveria o cadastro.

### Origem
`Origem::Planilha { arquivo: nome do .eml/.msg ou "texto colado", linha: nº do bloco }`, na tabela `emissao_importada`. A coluna `linha` fica só com o número do bloco, e não há migração.

### App
**Comando `ler_email`**, em `crates/app/src/comandos/planilha.rs`, no `generate_handler!` com o caminho completo, com cofre como `ler_planilha`:
- recebe `nome` + `bytes` ou `texto`, além de `empresaId`, `servicoId` e `mes`;
- consulta os CEPs em sequência, emitindo o evento `andamento-email` (padrão de `comandos/atualizacao.rs:49`);
- resolve e valida;
- devolve `ItemPlanilhaDto`, que ganha `avisos` e `tomadorNovo`;
- nunca registra o corpo.

**Interface:**
- Constante `FILTRO_PLANILHA_EMAIL` com as extensões das planilhas mais eml e msg. A `FILTRO_PLANILHA` é compartilhada com outras telas e fica como está.
- Seletor de serviço.
- Campo de texto com **Ler texto colado**, que fica desabilitado com o campo vazio.
- Andamento.
- Avisos na situação da linha.
- **Ler de novo:** relê o arquivo pelo `reler_arquivo_escolhido`; no texto colado, relê o campo.

## Alternativas descartadas
| Alternativa | Por que não |
|---|---|
| Parser MIME, QP, base64 e HTML→texto escritos no projeto | Muito código e casos de borda de charset; o `mail-parser` é Rust puro e não traz dependências pesadas |
| Trait `ConsultaCep` injetada no `nfse-emissor` | O mapa de CEP já resolvido simula a consulta sem trait nem async, e mantém a rede fora do núcleo |
| Consultar o CEP na emissão, e não na leitura | O R12 exige o erro na leitura, e a linha não pode ficar pronta sem município |
| Contador `#n` da planilha para blocos iguais | O histórico citado em respostas repete blocos e geraria nota em dobro (decisão do usuário) |
| Reconhecer NOME, TELEFONE e E-MAIL já nesta versão | Fora dos exemplos reais (decisão do usuário) |
| Gravar a origem do e-mail numa tabela nova | `Origem::Planilha` com o nome do arquivo e o número do bloco basta, sem migração |

## Mudanças
| Módulo / arquivo | Mudança | Requisitos |
|---|---|---|
| `crates/nfse-emissor/Cargo.toml` | + `mail-parser`, + `cfb` | R3, R4 |
| `nfse-emissor/src/leitura_email.rs` (novo) | `.eml`/`.msg` → texto | R1, R3, R4 |
| `nfse-emissor/src/pedido_email.rs` (novo) | Leiautes de blocos e de texto corrido | R5, R6, R16 |
| `nfse-emissor/src/importacao_email.rs` (novo) | Resolução das linhas, CEP, repetição | R7–R15 |
| `nfse-emissor/src/importacao.rs` | Chave em função comum; `avisos` e `tomador_novo` no `ItemImportado` | R10, R11, R15 |
| `nfse-emissor/src/lib.rs` | `emitir_importado` cadastra o tomador novo | R11, R17 |
| `crates/app/src/comandos/planilha.rs`, `main.rs` | Comando `ler_email`, evento de andamento, DTO | R1, R2, R11, R12 |
| `crates/app/ui/index.html`, `ui/js/planilha.js`, `ui/js/componentes.js` | Filtro, campo de texto, serviço, andamento, avisos, Ler de novo | R1, R2, R8, R18 |
| `CHANGELOG.md`, documento dono da importação, `docs/arquitetura.md` | Registro da funcionalidade, do módulo e das dependências | — |

## Contratos alterados
- **IPC:**
  - comando novo `ler_email`;
  - evento novo `andamento-email`;
  - `ItemPlanilhaDto` ganha `avisos` e `tomadorNovo` (opcionais, com `#[serde(default)]`, para o `emitir_planilha` continuar aceitando o item da planilha).
- **Banco:** sem migração e sem coluna nova. `emissao_importada.arquivo` passa a guardar também nomes `.eml`/`.msg` e "texto colado".

## Estratégia de teste
| Requisito / CA | Tipo de teste | Onde |
|---|---|---|
| R1, R3 / CA1, CA2, corrompido | unitário, `.eml` fictício inline | `leitura_email::testes` |
| R4 / CA3, CA4 | unitário, `.msg` montado em memória com `cfb` | `leitura_email::testes` |
| R5 / CA5, CA6, desempenho com 100 blocos | unitário | `pedido_email::testes` |
| R6, R16 / CA7 (extração), CA8, CA17 | unitário | `pedido_email::testes` |
| Invariante da planilha | testes existentes da planilha sem mudança | `importacao`, `testes.rs` |
| R7–R10, R12–R15 / CA7, CA9–CA11, CA14–CA16, repetido | integração com banco em memória e mapa de CEP simulado | `nfse-emissor/src/testes.rs` |
| R10, R11, R17 / CA12, CA13, CA18 | integração com `SefinSimulada` | `nfse-emissor/src/testes.rs` |
| R2, R8, R18 | manual no app | `just run` |

## Riscos e mitigação
- **O HTML→texto do mail-parser pode juntar parágrafos.** O CA2 cobre esse caso; se falhar, quebrar `<p>`/`<br>` em linhas antes de converter.
- **`.msg` ANSI sem página de código:** usar Windows-1252.
- **Tomador novo gravado antes do envio:** se a emissão falhar depois, a pessoa fica cadastrada. É o mesmo comportamento do vínculo de hoje e não fere a idempotência.
- **CPF de terceiros no texto corrido** (assinatura, por exemplo): vale o primeiro documento válido, e o operador confere a linha.
- **Vazamento do corpo:** nenhum `log`/`eprintln` no caminho do e-mail. As mensagens de erro não citam trechos do texto.

## Rollback
Reverter os commits da branch. Não há migração e não há dado novo no banco além de pessoas e vínculos comuns.

## Decisões que viram ADR
- Nenhuma. A dependência nova é trocável e não muda formato de dados.
