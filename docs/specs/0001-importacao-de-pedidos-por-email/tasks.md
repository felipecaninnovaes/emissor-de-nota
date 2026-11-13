# 0001 — Tarefas

Cada tarefa: um commit, `just check` verde ao terminar, um teste que a prova.

- [x] **T1** — Leitura de `.eml` (texto puro, HTML, QP, base64, charset)
  - Requisitos: R1, R3
  - Arquivos: `crates/nfse-emissor/Cargo.toml` (+ `mail-parser`), `crates/nfse-emissor/src/leitura_email.rs`, `crates/nfse-emissor/src/lib.rs` (mod)
  - Teste: `leitura_email::testes::eml_quoted_printable_iso_8859_1` (CA1), `eml_so_html_base64` (CA2), `eml_corrompido`
  - Verificar: `just test -p nfse-emissor leitura_email`

- [x] **T2** — Leitura de `.msg` (Unicode, ANSI com página de código, HTML, só RTF)
  - Requisitos: R1, R4
  - Arquivos: `crates/nfse-emissor/Cargo.toml` (+ `cfb`), `crates/nfse-emissor/src/leitura_email.rs`
  - Teste: `leitura_email::testes::msg_unicode` (CA3), `msg_ansi_windows_1252`, `msg_so_rtf` (CA4), `msg_corrompido`
  - Verificar: `just test -p nfse-emissor leitura_email`

- [x] **T3** — Extração do leiaute de blocos com rótulos
  - Requisitos: R5
  - Arquivos: `crates/nfse-emissor/src/pedido_email.rs`, `crates/nfse-emissor/src/lib.rs` (mod)
  - Teste: `pedido_email::testes::tres_blocos_irregulares` (CA5), `rotulos_sem_acento_e_espacos` (CA6), `cem_blocos_em_menos_de_2s`
  - Verificar: `just test -p nfse-emissor pedido_email`

- [x] **T4** — Extração do texto corrido e "nenhum pedido"
  - Requisitos: R6, R16
  - Arquivos: `crates/nfse-emissor/src/pedido_email.rs`
  - Teste: `pedido_email::testes::texto_corrido_ignora_cnpj_da_prestadora` (CA7, extração), `texto_corrido_pula_dv_invalido` (CA8), `sem_pedido` (CA17)
  - Verificar: `just test -p nfse-emissor pedido_email`

- [x] **T5** — Refatoração: montagem da chave de conteúdo em função comum
  - Requisitos: R15 (preparação); invariante da planilha
  - Arquivos: `crates/nfse-emissor/src/importacao.rs`
  - Teste: os testes existentes da planilha (`planilha_emite_e_nao_repete_ao_reimportar` e os de `importacao::testes`) continuam verdes, sem mudança
  - Verificar: `just test -p nfse-emissor planilha` e `just test -p nfse-emissor importacao`

- [x] **T6** — Resolução das linhas do e-mail (serviço, mês, descrição, tomador, CEP, repetição, validação)
  - Requisitos: R7, R8, R9, R10, R12, R13, R14, R15
  - Arquivos: `crates/nfse-emissor/src/importacao_email.rs`, `crates/nfse-emissor/src/importacao.rs` (`avisos`, `tomador_novo`), `crates/nfse-emissor/src/testes.rs`
  - Teste: `testes::email_tomador_da_filial_valor_e_mes` (CA7), `email_sem_periodo_usa_mes_da_tela` (CA9), `email_sem_servico` (CA10), `email_descricao_do_bloco_e_padrao` (CA11), `email_pessoa_nova_sem_municipio` (CA14), `email_valor_invalido_igual_planilha` (CA15), `email_reimportado_ja_emitido` (CA16), `email_bloco_repetido`, `email_aviso_dados_diferentes`
  - Verificar: `just test -p nfse-emissor email_`

- [x] **T7** — Emissão com tomador novo e origem do e-mail
  - Requisitos: R10, R11, R17
  - Arquivos: `crates/nfse-emissor/src/lib.rs` (`emitir_importado`), `crates/nfse-emissor/src/importacao.rs` (`pedido()`), `crates/nfse-emissor/src/testes.rs`
  - Teste: `testes::email_emite_com_cadastro_existente_sem_mudar` (CA12), `email_cadastra_pessoa_nova_ao_emitir` (CA13), `email_origem_guarda_so_arquivo_e_linha` (CA18)
  - Verificar: `just test -p nfse-emissor email_`

- [x] **T8** — Comando `ler_email` com consulta de CEP e evento de andamento
  - Requisitos: R1, R2, R11, R12
  - Arquivos: `crates/app/src/comandos/planilha.rs`, `crates/app/src/main.rs` (`generate_handler!`)
  - Teste: unitário do DTO (`avisos` e `tomadorNovo` com `serde(default)`), em `comandos::planilha::testes`
  - Verificar: `just test -p emissor-nfse planilha`

- [ ] **T9** — Interface: arquivo de e-mail, texto colado, serviço, andamento, avisos, Ler de novo; CHANGELOG e docs
  - Requisitos: R1, R2, R8, R18
  - Arquivos: `crates/app/ui/index.html`, `crates/app/ui/js/planilha.js`, `crates/app/ui/js/componentes.js`, `CHANGELOG.md`, documento dono da importação, `docs/arquitetura.md`
  - Teste: manual no app (`just run`), com e-mails fictícios: `.eml`, `.msg`, texto colado, sem serviço, andamento do CEP, Ler de novo, reimportar depois de emitir
  - Verificar: `just check`

## Notas de desvio
<!-- T#: o que mudou em relação ao plano e por quê -->
- T5: além da chave (`base_da_chave`, `chave_da_ocorrencia`), extraída também a busca do tomador cadastrado (`tomador_cadastrado`), que o e-mail usa igual à planilha.
- T6: `PedidoEmissao.tomador` só aceita pessoa cadastrada, então a prévia da linha passou para `Emissor::validar_importado` (backend), que confere o tomador novo como vai ser cadastrado e tira o não vinculado; o `ler_planilha` do app passou a usá-la, com o mesmo comportamento. A consulta de CEP devolve também logradouro e bairro (`EnderecoDoCep`), usados quando faltam no e-mail. A repetição só conta entre linhas sem erro, para não esconder o erro de valor. `emitir_importado` recusa a linha com tomador novo até a T7.
- T7: os testes ficaram `email_cpf_cadastrado_vale_o_cadastro_na_emissao` (CA12), `email_pessoa_nova_cadastrada_so_ao_emitir` (CA13), `email_pessoa_cadastrada_entre_leitura_e_emissao` (pessoa cadastrada entre a leitura e a emissão: só vincula) e `email_origem_guarda_so_arquivo_e_linha` (CA18). `pedido()` não mudou: o tomador novo é cadastrado em `Emissor::cadastrar_tomador_novo`, depois de conferir a linha com `validar_importado`, para não cadastrar a pessoa de uma nota que não sairia. Para o CA18, `Banco::origem_importada` (só com a feature `teste`) lê a origem gravada.
- T8: os campos novos do DTO ficaram `avisos` e `tomador_novo`, em snake_case como os outros campos do item (só os argumentos de primeiro nível do `invoke` são camelCase). O texto colado chega em `textoColado` (`texto` já é a função de erro do módulo). A conversão de ida e volta do item ficou em `dto_do_item` e `importado_do_dto`, comuns à planilha e ao e-mail; testes `comandos::planilha::testes`.
