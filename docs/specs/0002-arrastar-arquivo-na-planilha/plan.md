# 0002 — Plano técnico

- **Spec:** [spec.md](spec.md)
- **Status:** aprovado

## Abordagem
O Tauri 2 já entrega o arrastar como evento da janela (`WindowEvent::DragDrop`; `dragDropEnabled` é o padrão, nada muda no `tauri.conf.json`) e o repassa à interface como `tauri://drag-enter`, `tauri://drag-over`, `tauri://drag-leave` e `tauri://drag-drop` (liberados por `core:default`). O backend escolhe e registra o arquivo; a interface decide se o evento vale para a tela aberta.

1. **Backend (`crates/app/src/comandos/dialogo.rs`).**
   - Função pura `arquivo_solto(caminhos: &[PathBuf]) -> Result<PathBuf, String>`:
     - mais de um caminho, ou um que não é arquivo (pasta) → "Solte um arquivo só.";
     - extensão fora de `EXTENSOES_SOLTAR` (`xlsx xlsm xls ods csv eml msg`), comparada em minúsculas → "Tipo de arquivo não aceito: <nome>".
   - `ao_soltar(janela, caminhos)`: se aceito, `registrar` na mesma lista do diálogo (`Estado::arquivos_escolhidos`) e emite `arquivo-solto` com `{ id, nome }`; se recusado, `{ erro }`. Não manda bytes nem caminho.
   - Nenhum comando novo: a interface lê o conteúdo por `reler_arquivo_escolhido(id)`, que só aceita números registrados.
2. **`crates/app/src/main.rs`:** `on_window_event` no `Builder` repassa o `DragDrop::Drop` da janela `main` para `ao_soltar`.
3. **Interface (`crates/app/ui/js/planilha.js`).**
   - Só vale com a seção `planilha` visível (com o cofre fechado ela não está).
   - Flag `planilhaLendo` durante `lerPlanilha` e a releitura do arquivo solto; com ela ou `planilhaEmAndamento`, soltar avisa "Espere terminar…".
   - `tauri://drag-enter` põe a classe `soltando` na seção (não durante a emissão); `drag-leave` e `drag-drop` a tiram.
   - `arquivo-solto`: com `erro`, aviso e nada muda; senão relê pelo `id`, mostra o nome no campo, descarta o texto colado e lê. O **Ler de novo** já relê pelo `id`.
4. **`index.html` + `estilo.css`:** aviso "Solte para ler o arquivo" dentro da seção, visível só com `.soltando`.
5. **Docs:** `CHANGELOG.md`, `docs/usuario.md` (arrastar; Outlook e webmail não entregam arquivo) e `docs/arquitetura.md` se a fronteira dos arquivos estiver descrita lá.

A lista de extensões fica em Rust (`EXTENSOES_SOLTAR`) e no JS (`FILTRO_PLANILHA_EMAIL`, para o diálogo); não é regra fiscal, e cada uma aponta para a outra em comentário.

## Riscos
- O evento de soltar do WebKitGTK e do macOS não foi conferido na prática; o roteiro manual cobre o Windows.
- Arrastar do Outlook clássico (arquivo virtual) não deve gerar evento com caminho; confirmar no teste manual.
