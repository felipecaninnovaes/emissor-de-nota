# 0002 — Tarefas

Cada tarefa: um commit, `just check` verde ao terminar, um teste que a prova.

- [ ] ~~**T0** — `uf_do_ibge` desce para `nfse_core::municipios`~~ — retirada: o `nfse-pdf` continua independente do `nfse-core`; `municipios` tem a sua própria `uf_do_ibge` (T1).

- [x] **T1** — Tabela de municípios embutida e gerador
  - Requisitos: R1
  - Arquivos: `crates/nfse-core/examples/gerar_municipios.rs`, `crates/nfse-core/dados/municipios.tsv`, `crates/nfse-core/src/municipios.rs`, `docs/desenvolvimento.md`
  - Gerar: `cargo run -p nfse-core --example gerar_municipios -- D:\USUARIO\Downloads\RELATORIO_DTB_BRASIL_2025_MUNICIPIOS.xls`
  - Teste: `municipios::testes::sao_paulo_pelo_codigo_e_fonte` (CA1), `municipios::testes::tabela_embutida_e_valida` (CA2: 7 dígitos, ordem sem repetição, total igual ao impresso pelo gerador)
  - Verificar: `just test -p nfse-core municipios`

- [x] **T2** — Busca por nome, nome + UF e código
  - Requisitos: R2, R7
  - Arquivos: `crates/nfse-core/src/municipios.rs`
  - Teste: `municipios::testes::*` — CA3 ("ribeirao preto" → 3543402 primeiro), CA4 (com e sem acento/maiúscula), CA5 (código), CA6 ("bom jesus" com várias UFs; "bom jesus rs" com o do RS primeiro), CA7 (vazio e sem correspondência), "sao paulo" → São Paulo/SP primeiro, "embu guacu", termo de uma letra respeita o limite
  - Verificar: `just test -p nfse-core municipios`

- [x] **T3** — Comando `buscar_municipios`
  - Requisitos: R2, R3
  - Arquivos: `crates/app/src/comandos/municipios.rs`, `crates/app/src/comandos/mod.rs`, `crates/app/src/main.rs`
  - Teste: `comandos::municipios::testes::busca_pelo_codigo_devolve_fonte_e_itens`
  - Verificar: `just test -p emissor-nfse municipios`

- [x] **T4** — Interface dos três campos, CHANGELOG e docs
  - Requisitos: R3, R4, R5, R6, R7
  - Arquivos: `crates/app/ui/index.html`, `crates/app/ui/js/formularios.js`, `componentes.js`, `servicos.js`, `consultas.js`, `emitir.js`, `notas.js`, `tomadores.js`, `empresas.js`, `CHANGELOG.md`, `docs/usuario.md`
  - Teste: manual no app (`just run`):
    - CA8: Emitir → "ribeirao" no município da prestação → escolher Ribeirão Preto · SP → campo 3543402, "Ribeirão Preto/SP" abaixo, revisão com local 3543402
    - CA6/R7: "bom jesus rs" lista o do RS primeiro, com a UF de cada um
    - CA9: tomador e empresa, buscar e escolher pelo nome, salvar, reabrir: mesmo código e nome abaixo
    - CA10: refazer, copiar e substituir nota com local diferente do município da empresa: nome abaixo do campo
    - CA11: 9999999 → aviso de fora da tabela; Revisar e Salvar seguem
    - CA12: "12345" ou um nome sem escolher → erro de formato junto do campo
    - R4: CEP, consulta de CNPJ e "Usar o endereço do tomador" atualizam o nome abaixo
  - Verificar: `just check`

- [ ] **T5** — Fechamento da spec
  - Requisitos: —
  - Arquivos: `docs/specs/0002-busca-de-municipio-por-nome/spec.md` (status `implementada`, questão em aberto fechada com a DTB 2025, histórico), `docs/arquitetura.md` só se couber
  - Verificar: `just check`

## Notas de desvio
<!-- T#: o que mudou em relação ao plano e por quê -->
- T0: retirada antes de começar; mover a função criaria a dependência `nfse-pdf` → `nfse-core`, e o PDF é independente por desenho.
