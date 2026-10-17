# Plano de usabilidade — responsividade, densidade, formulários e acessibilidade

Criado em 16/10/2026, depois da troca para a identidade "documento fiscal" (commit `e607490`).
Escopo: só a interface (`crates/app/ui/` e a janela em `tauri.conf.json`). Nada muda no núcleo fiscal.

## 1. Diagnóstico

As telas foram renderizadas com dados de exemplo realistas (razões sociais longas, quatro ações por linha, mensagens de rejeição) em quatro tamanhos de janela, medidos em **pixels lógicos** — o que o WebView2 enxerga depois da escala do Windows:

| Cenário | Área lógica | Situação |
| --- | --- | --- |
| Janela mínima atual | 1024 × 680 | Tabelas cortadas à direita; lote e planilha com botões quebrando |
| Notebook 1366×768 a 125% | 1093 × 614 (≈ 1093 × 566 sem a barra de tarefas) | **A janela mínima não cabe** (680 > 566); diálogo de serviço só com rolagem |
| Full HD a 150% (notebook 15") | 1280 × 720 | Ok na largura; diálogos longos rolam |
| Full HD a 100% | 1920 × 1080 | Conteúdo preso em 1100px com 500px vazios; linhas de tabela altas mesmo com espaço |

### 1.1 Janela e escala
- `minWidth 1024 / minHeight 680` é maior que a área útil de notebooks 1366×768 a 125% (escala padrão do Windows nesses aparelhos). A janela abre cortada e não encolhe.
- Fontes e espaços em `px` fixos: o zoom do WebView (Ctrl + / Ctrl −) funciona, mas não é guardado nem oferecido na interface; não há modo compacto para telas baixas.
- O IBM Plex não está embutido: cada máquina mostra a fonte que tiver (Segoe UI / Consolas), e as larguras mudam.

### 1.2 Tabelas (Notas, Lote, Planilha, Empresas, Tomadores, Serviços, Recorrentes)
- **As ações quebram em coluna**: quatro `botao pequeno` numa célula estreita viram uma pilha, e cada linha fica com ~125px de altura (a de Empresas chega a 140px). É a maior causa de "quebra desnecessária".
- Em 1024px a tabela de notas passa da largura do cartão: Valor, Situação e Ações ficam fora da tela, sem rolagem visível.
- Colunas sem largura definida: nomes longos quebram em 3–4 linhas (Empresas) e a coluna Certificado mostra o titular inteiro do certificado.
- Mensagens longas (rejeição E0312, "E-mail enviado…") entram embaixo do selo e esticam a linha.
- Cabeçalho não fica visível ao rolar; não há contagem nem total do período.

### 1.3 Barras de ferramentas e cabeçalho
- Montadas com `style="display:flex…"` em linha (48 estilos em linha no `index.html` e 10 no `app.js`). Em 1024px o "Emitir selecionadas" do Lote cai para a linha de baixo, longe do "Marcar pendentes", e o "Baixar modelo" da Planilha passa da borda do cartão.
- O seletor de empresa tem `min-width: 360px` e corta a razão social; a validade do certificado disputa espaço com ele.
- Em Full HD o conteúdo fica preso a 1100px.

### 1.4 Formulários e diálogos
- Grade de 12 colunas com spans fixos: o campo encolhe junto com o diálogo, e não pelo tipo de dado (Porta com 100px e Senha com 330px; CNPJ apertado ao lado do "Consultar").
- Rótulos longos quebram em duas linhas e **desalinham os campos da mesma fileira** ("Código tributação nacional", "Nome do remetente (em branco = nome da empresa da nota)").
- O diálogo de serviço tem 14 campos em uma coluna rolável sem cabeçalho nem rodapé fixos: em 614px de altura os botões Salvar/Cancelar ficam fora da vista.
- Validação por **toast que some em 4–8 s** ("Informe o valor.") em vez de mensagem junto do campo; nenhum campo é marcado como inválido.
- Sem indicação de obrigatório; sem máscara para CNPJ/CPF, CEP, telefone e valor (o operador digita pontos e barras à mão, ou não).
- O ajuda-texto às vezes vira parte do rótulo ("Alíquota (%) — só se exigida", "E-mail (vai na nota)").

### 1.5 Acessibilidade
- **66 dos 81 `<label>` não estão associados ao campo** (sem `for` e sem envolver o controle): o leitor de tela anuncia "caixa de edição" sem nome, e clicar no rótulo não foca o campo. Afeta todos os diálogos e o formulário de e-mail.
- Nenhum atributo ARIA no app. A busca de tomador tem teclado (↑ ↓ Enter Esc), mas o leitor de tela não sabe que há uma lista nem qual item está em foco.
- Toasts não são anunciados (sem `role="status"`/`role="alert"`) e erros importantes desaparecem sozinhos.
- Navegação: o item ativo é só visual (`.ativo`), sem `aria-current`; ao trocar de tela o foco fica no botão do menu e o leitor não anuncia o novo título.
- Modo de alto contraste do Windows: o item ativo da navegação usa `box-shadow` (some nesse modo); a barra de andamento e os selos dependem de fundo.
- Botões "Cancelar", "PDF" e "E-mail" repetidos em cada linha têm o mesmo nome para o leitor de tela; não dizem de qual nota são.
- `scrollIntoView({behavior:"smooth"})` ignora `prefers-reduced-motion`; o cursor "ocupado" não tem equivalente para tecnologia assistiva (`aria-busy`).
- Pontos fortes a manter: contraste de texto ≥ 4,5:1 nos dois temas, foco visível de 2px, `lang="pt-BR"`, `<dialog>` nativo (prende o foco e fecha com Esc), selos com palavra além da cor.

## 2. Metas

1. **Cabe e funciona de 960 × 560 até 2560 × 1440 lógicos**, nas escalas 100–175% do Windows, sem rolagem horizontal da página.
2. **Uma linha por registro** nas tabelas em ≥ 1280px; em telas menores, colunas secundárias somem antes de qualquer quebra.
3. **Campos dimensionados pelo dado** (CNPJ, CEP, valor, percentual) e rótulos de uma linha, alinhados em cada fileira.
4. **Erros junto do campo e persistentes**; o toast fica só para confirmações.
5. **WCAG 2.2 AA** no que se aplica a um app desktop: nome em todo controle, teclado completo, anúncios de mudança, foco visível, alto contraste do Windows.
6. Tudo com tokens e classes do design system, sem estilo em linha.

## 3. Plano em etapas

Cada etapa é um commit separado, testado no app real por você antes do próximo. A ordem prioriza o que hoje impede o uso (janela que não cabe, tabelas cortadas) e o que custa pouco (rótulos).

### Etapa 1 — Janela, escala e densidade · feita em 16/10/2026 (com a parte 5a)
- `tauri.conf.json`: `minWidth 960`, `minHeight 560`; abrir com 90% da área útil do monitor e lembrar tamanho/posição entre sessões (plugin `window-state` do Tauri).
- Converter tamanhos de fonte e espaços para `rem` (base 14px = 0,875rem) para que zoom e escala funcionem por igual.
- **Zoom da interface** em Configurações (90% / 100% / 110% / 125% / 140% / 150%) com Ctrl + / Ctrl − / Ctrl 0 e Ctrl + roda, guardado na tabela `configuracao` e aplicado pela variável `--zoom` no tamanho da fonte raiz (tudo em `rem`; não precisa de permissão extra do Tauri).
- **Densidade** Confortável / Compacta: uma variável `--densidade` multiplica paddings de cartão, célula e campo; a compacta entra sozinha quando a altura útil for < 700px.
- Embutir IBM Plex Sans, Sans Condensed e Mono (woff2, licença OFL) em `ui/fonts/` com `@font-face` — métricas iguais em todas as máquinas, sem depender da rede (a CSP continua `'self'`).
- Conteúdo fluido: `max-width` passa de 1100px para `min(100%, 1440px)`; tabelas ocupam a largura toda.

### Etapa 2 — Layout adaptável (casca, cabeçalho e barras) · feita em 17/10/2026
- Navegação lateral recolhível: em < 1180px vira faixa de 64px com as iniciais/abreviações das telas e o nome em `title`/`aria-label`; botão para expandir/recolher, estado guardado.
- Cabeçalho: seletor de empresa `flex: 1` com `min-width: 16rem` e busca por digitação; validade do certificado vira um carimbo curto ("CERT 23 DIAS") com o detalhe no `title`.
- Nova classe `.barra` para as barras de ferramentas (filtros à esquerda, ações à direita, `flex-wrap` com a ação primária sempre junto das secundárias), substituindo os `style=` de Notas, Lote, Planilha, Recorrentes, Tomadores, Serviços, Empresas e Configurações.
- Container queries (`container-type: inline-size` no `.conteudo` e nos cartões) em vez de media queries: o mesmo componente se ajusta dentro de um diálogo ou da tela cheia.

### Etapa 3 — Tabelas sem quebras · feita em 17/10/2026
- **Ações da linha**: a ação principal visível (PDF para autorizada, "Corrigir e emitir" para rejeitada, "Verificar" para incerta) e as demais num botão "Mais ▾" com menu (popover nativo + teclado). A linha volta a ter ~44px.
- `<colgroup>` com larguras por tipo: número/série/valor em `ch` e mono, situação com largura do maior carimbo, nome e descrição flexíveis com reticências e texto completo no `title`.
- Prioridade de colunas por container query: abaixo de 900px somem DPS e Descrição em Notas, Descrição no Lote e na Planilha, Município em Tomadores; o conteúdo vai para uma linha de detalhe expansível.
- Mensagens longas (rejeição, e-mail enviado) viram uma linha de detalhe expansível ou um ícone-texto "ver motivo", não um parágrafo dentro da célula.
- Cabeçalho fixo ao rolar (`position: sticky`), rodapé com contagem e total do período em Notas e Lote, e um `.tabela-rolagem` com rolagem horizontal visível como último recurso.
- Empresas: coluna Certificado mostra só validade e dias restantes (carimbo), titular no `title`.

### Etapa 4 — Formulários e diálogos · feita em 17/10/2026
- Estrutura do diálogo: cabeçalho fixo (título + empresa), corpo rolável, **rodapé fixo com as ações**; `max-height: calc(100dvh - 2rem)`.
- Larguras pelo dado, com classes `.campo-cnpj` (19ch), `.campo-cpf` (15ch), `.campo-cep` (10ch), `.campo-valor` (14ch), `.campo-pct` (8ch), `.campo-porta` (7ch), `.campo-ibge` (9ch); a grade passa a `repeat(auto-fill, minmax(…))` por fieldset e cai para 6 colunas em diálogos estreitos.
- Rótulos de uma linha (texto curto) e a explicação em `.ajuda` abaixo; fileiras com `align-items: end` para alinhar os campos mesmo se um rótulo quebrar. Exemplos: "Código tributação nacional" → rótulo "Cód. trib. nacional" + ajuda "6 dígitos, ex.: 171901"; "Nome do remetente (em branco = …)" → rótulo "Nome do remetente" + ajuda.
- Diálogo de serviço reorganizado em blocos com títulos (Identificação · ISSQN · Federais · Total de tributos), lado a lado em ≥ 1100px.
- **Validação junto do campo**: `aria-invalid`, borda e texto `vermelho`, mensagem ligada por `aria-describedby`, foco no primeiro campo com erro; resumo no topo do diálogo quando houver mais de um. A emissão deixa de usar toast para "Informe o valor".
- Obrigatórios marcados com "*" na legenda e explicado uma vez no rodapé do diálogo.
- Máscaras leves e tolerantes (formatam ao sair do campo, aceitam colar com ou sem pontuação): CNPJ/CPF (detecta pelo tamanho), CEP, telefone, valor (R$ com milhar), percentual.
- Teclado: Enter salva (exceto em textarea), Esc fecha pedindo confirmação se houver alteração, foco inicial no primeiro campo vazio, "Consultar" CNPJ também por Enter no campo.

### Etapa 5 — Acessibilidade
- Associar os 66 rótulos (`for`/`id`), dar `aria-labelledby` aos diálogos e `<caption>` (visualmente oculto) às tabelas; `scope="col"` nos cabeçalhos.
- Busca de tomador no padrão **combobox ARIA** (`role="combobox"`, `aria-expanded`, `aria-controls`, `role="listbox"`/`option`, `aria-activedescendant`); trocar o `blur` com `setTimeout` por fechamento ao perder o foco do grupo.
- Região viva única: `role="status"` para confirmações, `role="alert"` para erros; erros não somem sozinhos (botão fechar).
- Navegação: `<nav aria-label="Telas">`, `aria-current="page"` no item ativo, foco no `<h1>` da tela ao trocar.
- Botões de linha com nome completo para leitor de tela (`aria-label="Cancelar nota 1234"`).
- Andamento do lote com `role="progressbar"` e `aria-valuenow/max`; `aria-busy` na área durante chamadas à Sefin.
- `@media (forced-colors: active)`: item ativo com `outline`, carimbos e barra com `border` em `CanvasText`, foco em `Highlight`.
- `@media (prefers-reduced-motion: reduce)`: rolagem sem animação.
- Alvos de clique ≥ 24×24px (WCAG 2.5.8): conferir `botao pequeno` e as caixas de marcação das tabelas.

### Etapa 6 — Fluidez do trabalho diário
- Atalhos (com lista em Configurações e no `title` dos botões): Alt+1…9 telas, Ctrl+N nova nota, Ctrl+Enter revisar/emitir, Ctrl+F busca da tela, F5 atualizar lista.
- Lembrar a última tela, o mês escolhido em Notas/Lote e a empresa selecionada.
- Estados vazios com a ação que resolve ("Nenhum serviço cadastrado — Novo serviço").
- Depois de emitir: resultado com PDF, e-mail e "Emitir outra para este tomador" (mantém tomador e serviço).

## 4. Como verificar cada etapa

- **Matriz de tamanhos** (pixels lógicos): 960×560, 1093×566, 1280×690, 1536×816, 1920×1032, 2560×1392 — cada tela, nos temas Papel e Carbono, com os dados de exemplo longos. As capturas deste diagnóstico foram feitas com o Edge sem janela sobre cópias estáticas do `index.html`; o mesmo roteiro pode virar um script em `crates/app/ui/teste/` (fora do pacote do app).
- **Teclado**: cada fluxo (emitir, cancelar, lote, planilha, cadastrar tomador) sem mouse.
- **Leitor de tela**: Narrador do Windows e NVDA na emissão e nos diálogos de tomador e serviço.
- **Alto contraste**: Windows → Temas de contraste → "Deserto" e "Noite".
- **Escala real**: Configurações do Windows a 125% e 150% com a janela no tamanho mínimo.
- Critério de pronto: nenhuma rolagem horizontal da página, nenhuma linha de tabela com mais de uma linha de texto em ≥ 1280px (exceto quando o detalhe é aberto), todos os controles com nome, nenhum erro de validação só em toast.

## 5. Ordem sugerida e tamanho

| Etapa | Impacto | Esforço | Observação |
| --- | --- | --- | --- |
| 1 Janela, escala, densidade | Alto (hoje não cabe em notebooks a 125%) | Médio | Precisa de um comando Tauri para zoom e do plugin window-state |
| 5a Rótulos e nomes (só a parte de `for`/`id`, `aria-current`, região viva) | Alto | Baixo | Feita junto da etapa 1: 66 rótulos associados, diálogos nomeados, erros com `role="alert"`, foco no título ao trocar de tela, toast de erro fica até fechar |
| 3 Tabelas | Alto (uso diário) | Médio | Maior ganho visual |
| 4 Formulários | Alto | Alto | Mexe em todos os diálogos e no `app.js` de validação |
| 2 Layout adaptável | Médio | Médio | |
| 5b Combobox, alto contraste, progresso | Médio | Médio | |
| 6 Fluidez | Médio | Baixo–médio | |

O design system recebe as mesmas classes novas (`.barra`, `.campo-*`, menu de ações, tabela com detalhe, diálogo com rodapé fixo) a cada etapa, para continuar sendo a referência.
