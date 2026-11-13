# 0002 — Arrastar o arquivo para a tela Importar planilha

- **Status:** aprovada
- **Autor:** Felipe CN
- **Data:** 2026-11-12
- **ADRs relacionados:** [0001 — interface sem bundler](../../decisoes/0001-interface-sem-bundler.md)

## Contexto e problema
Com a spec 0001, a tela **Importar planilha** lê planilhas e e-mails (`.eml`/`.msg`). Quem recebe um pedido costuma ter o arquivo aberto no Explorador de Arquivos e esperava poder arrastá-lo para a tela. Hoje nenhuma tela do app aceita arrastar: é preciso usar **Escolher arquivo…** e procurar o arquivo de novo no diálogo.

## Objetivos
- Soltar um arquivo vindo do gerenciador de arquivos na tela Importar planilha tem o mesmo efeito de escolhê-lo pelo botão, inclusive o **Ler de novo**.
- Sem abrir um caminho para a interface ler qualquer arquivo do disco.

## Fora de escopo
- Arrastar nas outras telas (importação de tomadores e de serviços, certificado etc.).
- Arrastar o e-mail direto do Outlook clássico, do novo Outlook ou do webmail: eles não entregam um arquivo de verdade (o Outlook clássico entrega um "arquivo virtual"). O operador salva o e-mail como `.eml`/`.msg` e arrasta o arquivo, ou cola o texto. Fica documentado em `docs/usuario.md`.
- Arrastar texto ou trecho de e-mail para o campo de texto colado.
- Soltar vários arquivos para importar de uma vez.

## Requisitos
| ID | Requisito | Prioridade |
|---|---|---|
| R1 | Soltar um arquivo `.xlsx`, `.xlsm`, `.xls`, `.ods`, `.csv`, `.eml` ou `.msg` na tela Importar planilha lê o arquivo como o botão **Escolher arquivo…**: mostra o nome no campo, descarta o texto colado anterior e lê a planilha ou o e-mail. | obrigatório |
| R2 | **Ler de novo** relê do disco o arquivo solto, como faz com o escolhido no diálogo. | obrigatório |
| R3 | O caminho do arquivo solto chega ao backend pelo evento de soltar do sistema (janela do Tauri), e não pela interface. O backend o registra na mesma lista dos arquivos escolhidos no diálogo e a interface recebe só o número e o nome. Nenhum comando novo aceita caminho vindo da interface. | obrigatório |
| R4 | Soltar mais de um arquivo, uma pasta ou um arquivo de extensão não aceita não lê nada, mostra aviso ("Solte um arquivo só." / "Tipo de arquivo não aceito: …") e mantém o que já estava carregado. | obrigatório |
| R5 | Enquanto um arquivo é arrastado sobre a tela Importar planilha, a tela inteira fica destacada (classe CSS, sem estilo em linha) com o texto "Solte para ler o arquivo"; o destaque some ao soltar ou ao sair da janela. | obrigatório |
| R6 | Soltar durante uma emissão ou com uma leitura em curso não lê nada e avisa "Espere terminar…"; durante a emissão a tela não fica destacada. | obrigatório |
| R7 | Soltar com outra tela aberta não faz nada. | obrigatório |

## Critérios de aceite
- **CA1 (R1, R3)** — Dado o evento de soltar com um único arquivo `pedidos.xlsx` (ou `.eml`), quando o backend o recebe, então o caminho entra na lista de arquivos escolhidos e a interface recebe `{ id, nome }`; ao ler esse id, vem o conteúdo do arquivo e a tela mostra as linhas, como pelo botão.
- **CA2 (R2)** — Dado um arquivo solto e depois alterado no disco, quando o operador clica em **Ler de novo**, então a tela mostra o conteúdo novo.
- **CA3 (R4)** — Dado o evento de soltar com dois arquivos, com uma pasta ou com `nota.pdf`, quando o backend o recebe, então nada é registrado e a interface recebe a mensagem de recusa; a tela mantém o arquivo anterior.
- **CA4 (R3)** — Dado qualquer extensão em maiúsculas (`PEDIDOS.CSV`), quando solto, então é aceito como a minúscula.
- **CA5 (R5)** — Dado um arquivo arrastado sobre a janela com a tela Importar planilha aberta, então a tela ganha a classe de destaque; ao sair da janela ou soltar, a classe sai.
- **CA6 (R6)** — Dada uma emissão em curso, quando um arquivo é solto, então nada é lido e aparece "Espere terminar…".
- **CA7 (R7)** — Dada a tela Notas emitidas aberta, quando um arquivo é solto, então nada acontece.

CA1, CA3 e CA4 têm teste automático em Rust (a escolha e o registro do arquivo solto ficam numa função pura); CA2 reusa o teste de `reler_arquivo_escolhido`; CA5 a CA7 e o arrastar real ficam no roteiro de teste manual (o app não tem teste de interface).

## Casos de borda e erros
- Arquivo apagado entre soltar e ler: o mesmo erro "não foi possível ler …" do botão.
- Arquivo grande: o mesmo caminho do botão; sem limite novo.
- Soltar do Outlook, do navegador ou texto: nenhum arquivo chega; nada acontece (documentado).
- Cofre fechado: a tela Importar planilha não está acessível; soltar não faz nada.
- Dois soltar seguidos rápidos: o segundo cai em "Espere terminar…" se o primeiro ainda estiver lendo.

## Invariantes (o que não pode mudar)
- O botão **Escolher arquivo…**, o texto colado e o **Ler de novo** continuam como na spec 0001.
- `ler_arquivo_escolhido` e `reler_arquivo_escolhido` continuam só lendo caminhos registrados pelo diálogo ou, agora, pelo evento de soltar do sistema.
- A trava dos controles durante a emissão (`emitirTravando`) continua valendo.

## Restrições
- Segurança: o código da interface não pode fazer o backend ler um caminho arbitrário (R3).
- Windows, Linux (WebKitGTK) e macOS: usar o evento de soltar da janela do Tauri, não o `drop` do HTML.
- Sem dependência nova.

## Questões em aberto
- [ ] Confirmar na prática, no Windows, que arrastar do Outlook clássico não gera nenhum evento com arquivo.

## Histórico de mudanças
| Data | Mudança | Motivo |
|---|---|---|
| 2026-11-12 | Criação | Pedido de arrastar o e-mail ou a planilha para a tela |
