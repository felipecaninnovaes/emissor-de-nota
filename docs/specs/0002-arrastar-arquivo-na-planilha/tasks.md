# 0002 — Tarefas

Cada tarefa: um commit, `just check` verde ao terminar, um teste que a prova.

- [x] **T1** — Escolha e registro do arquivo solto no backend
  - Requisitos: R1, R3, R4
  - Arquivos: `crates/app/src/comandos/dialogo.rs`, `crates/app/src/main.rs`
  - Teste: `comandos::dialogo::testes::solto_um_arquivo_e_registrado` (CA1), `solto_recusa_dois_pasta_e_pdf` (CA3), `solto_aceita_extensao_maiuscula` (CA4); CA2 fica com `reler_le_o_conteudo_atual_do_arquivo`
  - Verificar: `just test -p emissor-nfse dialogo`

- [x] **T2** — Interface (destaque, avisos, leitura), CHANGELOG e docs
  - Requisitos: R1, R2, R4, R5, R6, R7
  - Arquivos: `crates/app/ui/js/planilha.js`, `crates/app/ui/index.html`, `crates/app/ui/estilo.css`, `CHANGELOG.md`, `docs/usuario.md`, `docs/arquitetura.md` (se couber)
  - Teste: manual no app (`just run`): CA5 a CA7, arrastar `.xlsx`/`.eml`, **Ler de novo**, dois arquivos, pasta, `.pdf`, `PEDIDOS.CSV`, Outlook clássico
  - Verificar: `just check`

## Notas de desvio
<!-- T#: o que mudou em relação ao plano e por quê -->
- T2 (revisão): soltar com um `<dialog>` aberto (confirmação da emissão, cadastro do tomador) também recebe "Espere terminar…", porque o diálogo bloqueia cliques mas não o evento de soltar do sistema, e trocar a planilha ali gravaria a origem errada na emissão. A trava de leitura passou a cobrir o **Escolher arquivo…** (opção `ocupado` de `ligarEscolhaArquivo`) e o **Ler de novo** desde o clique.
