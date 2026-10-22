// Tabelas: menu de ações, detalhe da linha, seleção com caixas de marcar e linha vazia.

import { $, $$, esc } from "./util.js";
import { irPara } from "./navegacao.js";

let sequenciaMenu = 0;

/**
 * Ações de uma linha de tabela. Até duas ficam visíveis; com mais, a primeira fica e as outras vão
 * para o menu "Mais". `acoes`: [{ acao, texto, perigo? }]; `i`: índice da linha (data-i);
 * `alvo`: o que a linha representa, para o leitor de tela ("nota 1234").
 */
export function acoesLinha(acoes, i, alvo) {
  if (!acoes.length) return "";
  const botao = (a) =>
    `<button type="button" class="botao pequeno${a.perigo ? " perigo" : ""}" data-acao="${a.acao}" data-i="${i}"
      aria-label="${esc(`${a.texto} — ${alvo}`)}">${esc(a.texto)}</button>`;
  // `menu: true` manda a ação para "Mais" mesmo com poucas ações (a coluna não tem largura para dois botões longos)
  if (acoes.length <= 2 && !acoes.some((a) => a.menu)) return `<div class="acoes-linha">${acoes.map(botao).join("")}</div>`;
  const id = `menu-linha-${++sequenciaMenu}`;
  const itens = acoes
    .slice(1)
    .map(
      (a) => `<button type="button" role="menuitem" class="${a.perigo ? "perigo" : ""}" data-acao="${a.acao}"
        data-i="${i}">${esc(a.texto)}</button>`,
    )
    .join("");
  return `<div class="acoes-linha">${botao(acoes[0])}<button type="button" class="botao pequeno mais"
      popovertarget="${id}" aria-haspopup="menu" aria-label="${esc(`Mais ações — ${alvo}`)}">Mais ▾</button>
    <div class="menu" id="${id}" popover role="menu" aria-label="${esc(`Ações — ${alvo}`)}">${itens}</div></div>`;
}

// O menu abre colado ao botão "Mais" (acima dele se não couber embaixo) com o foco no primeiro item.
document.addEventListener(
  "toggle",
  (ev) => {
    const menu = ev.target;
    if (!menu.classList?.contains("menu") || ev.newState !== "open") return;
    const dono = document.querySelector(`[popovertarget="${menu.id}"]`);
    if (!dono) return;
    const r = dono.getBoundingClientRect();
    const { offsetWidth: largura, offsetHeight: altura } = menu;
    menu.style.left = `${Math.max(8, Math.min(r.right - largura, innerWidth - largura - 8))}px`;
    menu.style.top = `${r.bottom + 4 + altura > innerHeight - 8 ? Math.max(8, r.top - altura - 4) : r.bottom + 4}px`;
    menu.querySelector("button")?.focus();
  },
  true,
);

// Setas, Home e End percorrem os itens; escolher um item fecha o menu (o clique segue para a tabela).
document.addEventListener("keydown", (ev) => {
  const menu = ev.target.closest?.(".menu");
  if (!menu) return;
  const itens = $$("button", menu);
  const i = itens.indexOf(ev.target);
  const destino = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: itens.length - 1 }[ev.key];
  if (destino === undefined) return;
  ev.preventDefault();
  itens[(destino + itens.length) % itens.length].focus();
});
document.addEventListener(
  "click",
  (ev) => {
    const item = ev.target.closest(".menu button");
    if (item) item.closest(".menu").hidePopover();
  },
  true,
);

/**
 * Linha de detalhe que abre embaixo da linha principal: texto completo das colunas que ficam
 * cortadas ou escondidas em janelas estreitas, e o motivo de rejeição/avisos (`motivo`, HTML).
 * Devolve { botao, linha }: o botão vai na coluna "Detalhes" da linha principal.
 */
export function detalheLinha({ id, colunas, alvo, itens, motivo = "", erro = false, aberta = false }) {
  const lista = itens
    .filter(([, v]) => v != null && v !== "")
    .map(([rotulo, valor]) => `<dt>${esc(rotulo)}</dt><dd>${valor}</dd>`)
    .join("");
  const botao = `<button type="button" class="ver-detalhe${motivo ? " tem-motivo" : ""}" aria-expanded="${aberta}"
    aria-controls="${id}" aria-label="${esc(`Detalhes — ${alvo}`)}"></button>`;
  const linha = `<tr class="detalhe" id="${id}"${aberta ? "" : " hidden"}><td colspan="${colunas}">
    <dl class="detalhe-lista">${lista}</dl>
    <div class="detalhe-motivo${erro ? " erro" : ""}" data-motivo${motivo ? "" : " hidden"}>${motivo}</div></td></tr>`;
  return { botao, linha };
}

/**
 * Tabela com caixas de marcar (lote, planilha, importar serviços). `itens()` é a lista desenhada (a
 * caixa diz o índice em `data-i`); o botão de ação mostra quantos estão marcados. Com `emAndamento`,
 * o botão fica desligado sem marcados ou durante o envio, assim como o "Marcar" (`marcar`).
 */
export function selecaoEmTabela({ lista, itens, botao, rotulo, todos, marcar, emAndamento }) {
  const caixas = () => $$(`${lista} input[type=checkbox]`);
  const marcados = () =>
    caixas()
      .filter((c) => c.checked)
      .map((c) => itens()[Number(c.dataset.i)]);
  const atualizar = () => {
    const n = marcados().length;
    const b = $(botao);
    b.textContent = n ? `${rotulo} (${n})` : rotulo;
    if (!emAndamento) return;
    b.disabled = emAndamento() || !n;
    if (marcar) $(marcar).disabled = emAndamento();
  };
  const marcarTodos = (sim) => {
    for (const c of caixas()) if (!c.disabled) c.checked = sim;
    atualizar();
  };
  $(lista).addEventListener("change", atualizar);
  $(todos).addEventListener("change", (ev) => marcarTodos(ev.target.checked));
  if (marcar) $(marcar).addEventListener("click", () => marcarTodos(true));
  return { marcados, atualizar };
}

function abrirDetalhe(botao, abrir) {
  const linha = document.getElementById(botao.getAttribute("aria-controls"));
  if (!linha) return;
  linha.hidden = !abrir;
  botao.setAttribute("aria-expanded", String(abrir));
}

document.addEventListener("click", (ev) => {
  const b = ev.target.closest(".ver-detalhe");
  if (b) abrirDetalhe(b, b.getAttribute("aria-expanded") !== "true");
});

/**
 * Linhas que ocupam a tabela toda (detalhe, rodapé, "nenhum registro") usam colspan; quando colunas
 * somem em janelas estreitas, um colspan maior que as colunas visíveis criaria colunas fantasmas.
 * O colspan acompanha o número de colunas visíveis do cabeçalho.
 */
function ajustarColspan(tabela) {
  const visiveis = $$("thead th", tabela).filter((th) => getComputedStyle(th).display !== "none").length;
  for (const td of $$("tbody td[colspan], tfoot td[colspan]", tabela)) {
    if (td.colSpan !== visiveis) td.colSpan = visiveis;
  }
}
const observarLargura = new ResizeObserver((entradas) => {
  for (const e of entradas) ajustarColspan($("table", e.target));
});
const observarLinhas = new MutationObserver((mudancas) => {
  for (const tabela of new Set(mudancas.map((m) => m.target.closest("table")))) if (tabela) ajustarColspan(tabela);
});
for (const cartao of $$(".cartao.tabela")) {
  observarLargura.observe(cartao);
  for (const parte of $$("tbody, tfoot", cartao)) observarLinhas.observe(parte, { childList: true });
}

/** Troca o motivo mostrado no detalhe de uma linha (usado durante o lote); `abrir` já mostra. */
export function definirMotivo(tr, html, { erro = false, abrir = false } = {}) {
  const botao = $(".ver-detalhe", tr);
  const caixa = botao && $("[data-motivo]", document.getElementById(botao.getAttribute("aria-controls")));
  if (!caixa) return;
  caixa.innerHTML = html;
  caixa.hidden = !html;
  caixa.classList.toggle("erro", erro);
  botao.classList.toggle("tem-motivo", !!html);
  if (abrir && html) abrirDetalhe(botao, true);
}

/** Linha de "nenhum registro" com a ação que resolve: clica num botão da tela ou vai para outra tela. */
export function linhaVazia(colunas, texto, acao) {
  const alvo = acao?.clicar ? `data-clicar="${acao.clicar}"` : `data-ir="${acao?.ir}"`;
  const botao = acao ? ` <button type="button" class="botao pequeno" ${alvo}>${esc(acao.texto)}</button>` : "";
  return `<tr><td colspan="${colunas}" class="vazio">${esc(texto)}${botao}</td></tr>`;
}
document.addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-clicar], [data-ir]");
  if (!b) return;
  if (b.dataset.clicar) $(b.dataset.clicar)?.click();
  else irPara(b.dataset.ir, true);
});
