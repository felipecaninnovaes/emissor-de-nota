// Interface do emissor: liga as telas de index.html aos comandos Tauri (src/comandos.rs).
// Sem bundler: módulo ES puro. Argumentos de primeiro nível dos comandos vão em camelCase
// (convenção do Tauri); os campos dentro das structs seguem o serde (snake_case).

const invoke = window.__TAURI__.core.invoke;

const $ = (sel, raiz = document) => raiz.querySelector(sel);
const $$ = (sel, raiz = document) => [...raiz.querySelectorAll(sel)];

const estado = {
  empresas: [],
  empresa: null,
  servicos: [],
  tomador: null, // TomadorDto escolhido na tela de emitir
  pedido: null, // último pedido revisado
  reter: null, // { federais, iss } escolhido na revisão; null = cadastro do tomador
};

// ---------------------------------------------------------------- utilitários

function esc(texto) {
  return String(texto ?? "").replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
  );
}

/** Confirmação some em 4 s; erro fica até o operador fechar (e é anunciado como alerta). */
function toast(texto, erro = false) {
  const el = document.createElement("div");
  el.className = "toast" + (erro ? " erro" : "");
  const msg = document.createElement("span");
  msg.textContent = texto;
  el.append(msg);
  if (erro) {
    el.setAttribute("role", "alert");
    const fechar = document.createElement("button");
    fechar.type = "button";
    fechar.className = "fechar";
    fechar.textContent = "Fechar";
    fechar.addEventListener("click", () => el.remove());
    el.append(fechar);
  } else {
    setTimeout(() => el.remove(), 4000);
  }
  $("#toasts").append(el);
}

// ---------------------------------------------------------------- tabelas: ações e detalhe da linha

let sequenciaMenu = 0;

/**
 * Ações de uma linha de tabela. Até duas ficam visíveis; com mais, a primeira fica e as outras vão
 * para o menu "Mais". `acoes`: [{ acao, texto, perigo? }]; `i`: índice da linha (data-i);
 * `alvo`: o que a linha representa, para o leitor de tela ("nota 1234").
 */
function acoesLinha(acoes, i, alvo) {
  if (!acoes.length) return "";
  const botao = (a) =>
    `<button type="button" class="botao pequeno${a.perigo ? " perigo" : ""}" data-acao="${a.acao}" data-i="${i}"
      aria-label="${esc(`${a.texto} — ${alvo}`)}">${esc(a.texto)}</button>`;
  if (acoes.length <= 2) return `<div class="acoes-linha">${acoes.map(botao).join("")}</div>`;
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
function detalheLinha({ id, colunas, alvo, itens, motivo = "", erro = false, aberta = false }) {
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
function selecaoEmTabela({ lista, itens, botao, rotulo, todos, marcar, emAndamento }) {
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
function definirMotivo(tr, html, { erro = false, abrir = false } = {}) {
  const botao = $(".ver-detalhe", tr);
  const caixa = botao && $("[data-motivo]", document.getElementById(botao.getAttribute("aria-controls")));
  if (!caixa) return;
  caixa.innerHTML = html;
  caixa.hidden = !html;
  caixa.classList.toggle("erro", erro);
  botao.classList.toggle("tem-motivo", !!html);
  if (abrir && html) abrirDetalhe(botao, true);
}

/** Chama um comando mostrando o cursor de espera; o erro volta como texto. */
async function chamar(comando, args) {
  document.body.classList.add("ocupado");
  $("main")?.setAttribute("aria-busy", "true");
  try {
    return await invoke(comando, args);
  } finally {
    document.body.classList.remove("ocupado");
    $("main")?.removeAttribute("aria-busy");
  }
}

function mostrarErro(el, erro) {
  el.textContent = String(erro);
  el.hidden = false;
}

/** "150.00" → "150,00" com separador de milhar. */
function moeda(valor) {
  if (valor == null || valor === "") return "";
  const [inteiro, frac = "00"] = String(valor).split(".");
  return inteiro.replace(/\B(?=(\d{3})+(?!\d))/g, ".") + "," + frac.padEnd(2, "0");
}

/**
 * Soma valores do backend ("1250.00") em centavos, sem ponto flutuante; devolve no mesmo formato.
 * `campo(item)` lê o valor de cada item (nulo conta zero).
 */
function somarValores(itens, campo) {
  let centavos = 0n;
  for (const item of itens) {
    const [inteiro, frac = ""] = String(campo(item) ?? "0").split(".");
    centavos += BigInt(inteiro || "0") * 100n + BigInt((frac + "00").slice(0, 2));
  }
  return `${centavos / 100n}.${String(centavos % 100n).padStart(2, "0")}`;
}

/** "1.500,00" → "1500,00" (o backend aceita vírgula decimal). Vazio → null. */
function valorDigitado(texto) {
  const t = String(texto ?? "").trim().replace(/\s|R\$/g, "");
  if (!t) return null;
  return t.includes(",") ? t.replace(/\./g, "") : t;
}

/** "11.48" → "11,48" para campos de percentual. */
function percentualTela(p) {
  return p == null ? "" : String(p).replace(".", ",");
}

const soDigitos = (t) => String(t ?? "").replace(/\D/g, "");
const vazio = (t) => (String(t ?? "").trim() === "" ? null : String(t).trim());

function formatarDocumento(doc) {
  if (!doc) return "";
  if (doc.cnpj) {
    const c = doc.cnpj;
    return `${c.slice(0, 2)}.${c.slice(2, 5)}.${c.slice(5, 8)}/${c.slice(8, 12)}-${c.slice(12)}`;
  }
  if (doc.cpf) {
    const c = doc.cpf;
    return `${c.slice(0, 3)}.${c.slice(3, 6)}.${c.slice(6, 9)}-${c.slice(9)}`;
  }
  return "";
}

const VARIAVEIS_DESCRICAO = [
  ["mes_ref", "Mês de referência (09/2026)"],
  ["mes_ref_extenso", "Mês por extenso (setembro de 2026)"],
  ["ano_ref", "Ano de referência"],
  ["competencia", "Mês da emissão (10/2026)"],
  ["data_emissao", "Data da emissão"],
  ["tomador", "Nome do tomador"],
  ["empresa", "Nome da empresa"],
  ["valor", "Valor da nota (1.250,00)"],
  ["valor_liquido", "Valor menos as retenções (ISS retido exige a alíquota no serviço)"],
];

/** Fileira de etiquetas sob cada descrição; o clique insere `{variável}` onde está o cursor. */
function montarEtiquetasDeVariaveis() {
  for (const caixa of $$(".variaveis[data-alvo]")) {
    const campo = document.getElementById(caixa.dataset.alvo);
    for (const [nome, ajuda] of VARIAVEIS_DESCRICAO) {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "etiqueta-variavel";
      b.textContent = `{${nome}}`;
      b.title = ajuda;
      b.addEventListener("click", () => {
        const texto = `{${nome}}`;
        const ini = campo.selectionStart ?? campo.value.length;
        const fim = campo.selectionEnd ?? ini;
        campo.setRangeText(texto, ini, fim, "end");
        campo.focus();
        campo.dispatchEvent(new Event("input", { bubbles: true }));
      });
      caixa.append(b);
    }
  }
}
montarEtiquetasDeVariaveis();

function mesAnterior() {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function mesAtual() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

const NOME_AMBIENTE = { producao: "PRODUÇÃO", homologacao: "Homologação" };
const NOME_SITUACAO = {
  autorizada: "Autorizada",
  rejeitada: "Rejeitada",
  incerta: "Sem resposta",
  enviando: "Enviando",
  cancelada: "Cancelada",
  substituida: "Substituída",
};

/** Lê um <form> como objeto (checkbox → boolean). */
function lerForm(form) {
  const dados = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    dados[el.name] = el.type === "checkbox" ? el.checked : el.value;
  }
  return dados;
}

function preencherForm(form, dados) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in dados)) continue;
    if (el.type === "checkbox") el.checked = !!dados[el.name];
    else if (el.type !== "file") {
      el.value = dados[el.name] ?? "";
      if (el.dataset.formato) aplicarMascara(el);
    }
  }
}

// ---------------------------------------------------------------- formulários: formatos, máscaras e erros por campo

/** Erro de um campo específico: aparece embaixo dele em vez de no aviso geral do diálogo. */
class ErroCampo extends Error {
  constructor(campo, mensagem) {
    super(mensagem);
    this.campo = campo;
  }
}

/** Dígito verificador do CNPJ (vale também para o CNPJ alfanumérico: cada caractere vale código − 48). */
function digitoCnpj(base) {
  let soma = 0;
  let peso = 2;
  for (let i = base.length - 1; i >= 0; i--) {
    soma += (base.charCodeAt(i) - 48) * peso;
    peso = peso === 9 ? 2 : peso + 1;
  }
  const resto = soma % 11;
  return resto < 2 ? 0 : 11 - resto;
}

function cnpjValido(texto) {
  const c = String(texto).replace(/[^0-9A-Za-z]/g, "").toUpperCase();
  if (!/^[0-9A-Z]{12}\d{2}$/.test(c) || /^(\d)\1{13}$/.test(c)) return false;
  return digitoCnpj(c.slice(0, 12)) === Number(c[12]) && digitoCnpj(c.slice(0, 13)) === Number(c[13]);
}

function cpfValido(texto) {
  const c = soDigitos(texto);
  if (c.length !== 11 || /^(\d)\1{10}$/.test(c)) return false;
  for (const n of [9, 10]) {
    let soma = 0;
    for (let i = 0; i < n; i++) soma += Number(c[i]) * (n + 1 - i);
    if (((soma * 10) % 11) % 10 !== Number(c[n])) return false;
  }
  return true;
}

const mascaraCnpj = (t) => {
  const c = String(t).replace(/[^0-9A-Za-z]/g, "").toUpperCase();
  return c.length === 14 ? `${c.slice(0, 2)}.${c.slice(2, 5)}.${c.slice(5, 8)}/${c.slice(8, 12)}-${c.slice(12)}` : t;
};
const mascaraCpf = (t) => {
  const c = soDigitos(t);
  return c.length === 11 ? `${c.slice(0, 3)}.${c.slice(3, 6)}.${c.slice(6, 9)}-${c.slice(9)}` : t;
};
/** Valor digitado ("1500", "1.500,5", "1500.50") → número, ou NaN. */
const numeroDigitado = (t) => Number(String(valorDigitado(t) ?? "").replace(",", "."));
const exatoDigitos = (n, msg) => ({ mascara: soDigitos, valida: (v) => soDigitos(v).length === n || msg });
const emailValido = (e) => /^[^\s@;,]+@[^\s@;,]+\.[^\s@;,]+$/.test(e);

/**
 * Formatos dos campos marcados com data-formato: `mascara` arruma o texto ao sair do campo (aceita
 * colar com ou sem pontuação); `valida` devolve true ou a mensagem de erro. Campo vazio não passa
 * por aqui (quem decide se pode ficar vazio é o atributo required).
 */
const FORMATOS = {
  cnpj: { mascara: mascaraCnpj, valida: (v) => cnpjValido(v) || "CNPJ inválido: confira os 14 caracteres." },
  cpf: { mascara: mascaraCpf, valida: (v) => cpfValido(v) || "CPF inválido: confira os 11 dígitos." },
  cep: {
    mascara: (t) => {
      const c = soDigitos(t);
      return c.length === 8 ? `${c.slice(0, 5)}-${c.slice(5)}` : t;
    },
    valida: (v) => soDigitos(v).length === 8 || "CEP com 8 dígitos.",
  },
  ibge: exatoDigitos(7, "Código IBGE com 7 dígitos (ex.: 3511102)."),
  ctn: exatoDigitos(6, "6 dígitos, sem pontos (ex.: 171901)."),
  nbs: exatoDigitos(9, "NBS com 9 dígitos."),
  ctm: exatoDigitos(3, "Código municipal com 3 dígitos."),
  cst: exatoDigitos(2, "CST com 2 dígitos (ex.: 00)."),
  cst_ibs: exatoDigitos(3, "CST do IBS/CBS com 3 dígitos (ex.: 000)."),
  class_trib: exatoDigitos(6, "cClassTrib com 6 dígitos (ex.: 000001)."),
  telefone: {
    mascara: (t) => {
      const c = soDigitos(t);
      if (c.length === 10) return `(${c.slice(0, 2)}) ${c.slice(2, 6)}-${c.slice(6)}`;
      if (c.length === 11) return `(${c.slice(0, 2)}) ${c.slice(2, 7)}-${c.slice(7)}`;
      return t;
    },
    valida: (v) => [10, 11].includes(soDigitos(v).length) || "Telefone com DDD: 10 ou 11 dígitos.",
  },
  email: { mascara: (t) => t.trim(), valida: (v) => emailValido(v.trim()) || "E-mail inválido." },
  emails: {
    mascara: (t) =>
      t
        .split(/[;,\s]+/)
        .filter(Boolean)
        .join("; "),
    valida: (v) => {
      const ruins = v.split(/[;,\s]+/).filter((e) => e && !emailValido(e));
      return !ruins.length || `E-mail inválido: ${ruins.join(", ")}.`;
    },
  },
  valor: {
    mascara: (t) => {
      const n = numeroDigitado(t);
      return Number.isFinite(n) && n > 0 ? moeda(n.toFixed(2)) : t;
    },
    valida: (v) => {
      const n = numeroDigitado(v);
      return (Number.isFinite(n) && n > 0) || "Valor em reais, maior que zero (ex.: 1.250,00).";
    },
  },
  pct: {
    mascara: (t) => t.trim().replace(".", ","),
    valida: (v) => {
      const t = v.trim().replace(",", ".");
      const n = Number(t);
      return (/^\d{1,3}(\.\d{1,4})?$/.test(t) && n <= 100) || "Percentual de 0 a 100 (ex.: 2,5).";
    },
  },
  serie: { mascara: soDigitos, valida: (v) => /^\d{1,5}$/.test(soDigitos(v)) || "Série com 1 a 5 dígitos." },
  inteiro: { valida: (v) => /^\d+$/.test(v.trim()) || "Número inteiro, maior que zero." },
  porta: {
    valida: (v) => {
      const n = Number(v);
      return (Number.isInteger(n) && n >= 1 && n <= 65535) || "Porta de 1 a 65535 (ex.: 587).";
    },
  },
  documento: {}, // CNPJ ou CPF conforme o select indicado em data-tipo (ver formatoDe)
};

/** Formato efetivo de um campo; "documento" segue o tipo escolhido (e muda para CPF se vierem 11 dígitos). */
function formatoDe(el) {
  const nome = el.dataset.formato;
  if (nome !== "documento") return FORMATOS[nome];
  const tipo = document.getElementById(el.dataset.tipo);
  if (tipo && tipo.value === "cnpj" && soDigitos(el.value).length === 11 && !/[A-Za-z]/.test(el.value)) tipo.value = "cpf";
  return FORMATOS[tipo?.value === "cpf" ? "cpf" : "cnpj"];
}

function aplicarMascara(el) {
  const f = el.dataset.formato && formatoDe(el);
  if (f?.mascara && el.value.trim()) el.value = f.mascara(el.value);
}

function rotuloDe(el) {
  return (document.querySelector(`label[for="${el.id}"]`)?.textContent ?? el.name ?? "").trim();
}

/** Mensagem de erro do campo, ou "" se está bom. */
function erroDoCampo(el) {
  const v = el.type === "file" ? (el.files.length ? "x" : "") : el.value.trim();
  if (!v) {
    if (!el.required) return "";
    if (el.tagName === "SELECT") return "Escolha uma opção.";
    if (el.type === "file") return "Escolha o arquivo.";
    return "Preencha este campo.";
  }
  if (el.minLength > 0 && v.length < el.minLength) {
    return `Escreva pelo menos ${el.minLength} caracteres (faltam ${el.minLength - v.length}).`;
  }
  const f = el.dataset.formato && formatoDe(el);
  const r = f?.valida ? f.valida(el.value) : true;
  return r === true ? "" : r;
}

function marcarErro(el, mensagem) {
  const celula = el.closest(".grade > *") ?? el.parentElement;
  let p = $(".erro-campo", celula);
  if (!p) {
    p = document.createElement("p");
    p.className = "erro-campo";
    p.id = `${el.id || el.name}-erro`;
    celula.append(p);
  }
  p.textContent = mensagem;
  el.setAttribute("aria-invalid", "true");
  const descritos = new Set((el.getAttribute("aria-describedby") ?? "").split(" ").filter(Boolean));
  descritos.add(p.id);
  el.setAttribute("aria-describedby", [...descritos].join(" "));
}

function limparErro(el) {
  const p = $(".erro-campo", el.closest(".grade > *") ?? el.parentElement);
  if (p) {
    const resto = (el.getAttribute("aria-describedby") ?? "").split(" ").filter((id) => id && id !== p.id);
    if (resto.length) el.setAttribute("aria-describedby", resto.join(" "));
    else el.removeAttribute("aria-describedby");
    p.remove();
  }
  el.removeAttribute("aria-invalid");
}

function limparErros(raiz) {
  for (const el of $$('[aria-invalid="true"]', raiz)) limparErro(el);
}

/** Valida os campos visíveis de `raiz`; devolve [{ el, msg }] na ordem da tela. */
function validarCampos(raiz) {
  return $$("input, select, textarea", raiz)
    .filter((el) => !el.disabled && el.type !== "hidden" && el.type !== "checkbox" && el.getClientRects().length)
    .map((el) => {
      aplicarMascara(el);
      return { el, msg: erroDoCampo(el) };
    })
    .filter((e) => e.msg);
}

/** Marca os campos com erro e põe o foco no primeiro; com mais de um, resume no `resumoEl`. */
function mostrarErros(erros, resumoEl) {
  for (const { el, msg } of erros) marcarErro(el, msg);
  erros[0].el.focus();
  if (resumoEl && erros.length > 1) {
    mostrarErro(resumoEl, `Corrija ${erros.length} campos: ${erros.map((e) => rotuloDe(e.el)).join(", ")}.`);
  }
}

// Ao sair do campo: arruma o formato; um campo marcado com erro é conferido de novo (e desmarcado se ficou bom).
document.addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.dataset?.formato) aplicarMascara(el);
  if (el.getAttribute?.("aria-invalid") === "true") {
    const msg = erroDoCampo(el);
    if (msg) marcarErro(el, msg);
    else limparErro(el);
  }
});
document.addEventListener("input", (ev) => {
  const el = ev.target;
  if (el.getAttribute?.("aria-invalid") === "true" && !erroDoCampo(el)) limparErro(el);
  if (el.matches?.("[data-contador]")) atualizarContador(el);
});

/** Contador "n / máximo" embaixo de textos com limite (justificativa do cancelamento). */
function atualizarContador(el) {
  const celula = el.closest(".grade > *") ?? el.parentElement;
  let c = $(".contador", celula);
  if (!c) {
    c = document.createElement("span");
    c.className = "contador";
    el.after(c);
  }
  const n = el.value.length;
  c.textContent = `${n} / ${el.maxLength}`;
  c.classList.toggle("alerta", n > 0 && n < el.minLength);
}

/** Foco inicial: o primeiro campo vazio do diálogo (ou o primeiro campo, se já vem preenchido). */
function focarPrimeiroCampo(raiz) {
  const campos = $$("input, select, textarea", raiz).filter(
    (el) => !el.disabled && !el.readOnly && el.type !== "hidden" && el.getClientRects().length,
  );
  const vazioEl = campos.find((el) => el.tagName !== "SELECT" && el.type !== "checkbox" && !el.value);
  (vazioEl ?? campos[0])?.focus();
}

/**
 * Abre um <dialog> com formulário. `salvar(dados, form)` roda ao salvar, depois que os campos
 * passam na validação; se lançar ErroCampo, a mensagem vai para o campo; outro erro aparece em
 * `erroEl` e o diálogo continua aberto. Cancelar (ou Esc) com alterações pede confirmação.
 */
/**
 * Confirmação própria: o `confirm()` do navegador não funciona em todos os webviews (no Linux e no
 * macOS devolve "não" sem mostrar nada). Devolve uma promessa de true/false.
 */
function confirmar(texto, { ok = "Confirmar", cancelar = "Cancelar" } = {}) {
  const dlg = $("#dlg-confirmar");
  $("#confirmar-texto").textContent = texto;
  $("#confirmar-sim").textContent = ok;
  $("#confirmar-nao").textContent = cancelar;
  return new Promise((resolve) => {
    dlg.addEventListener("close", () => resolve(dlg.returnValue === "sim"), { once: true });
    dlg.returnValue = "nao";
    dlg.showModal();
    $("#confirmar-sim").focus();
  });
}

function abrirDialogo(dlg, erroEl, salvar) {
  const form = $("form", dlg);
  erroEl.hidden = true;
  limparErros(form);
  const empresa = $("[data-empresa-dialogo]", dlg);
  if (empresa) empresa.textContent = estado.empresa ? `${estado.empresa.nome} — ${estado.empresa.cnpj_formatado}` : "";
  for (const el of $$("[data-contador]", form)) atualizarContador(el);
  const inicial = JSON.stringify(lerForm(form));
  const mudou = () => JSON.stringify(lerForm(form)) !== inicial;
  /** Fecha o diálogo; se houve alterações, pergunta antes de descartar. */
  const descartar = async () => {
    if (!mudou() || (await confirmar("Descartar as alterações feitas?", { ok: "Descartar", cancelar: "Continuar editando" }))) {
      dlg.close();
    }
  };

  form.onsubmit = async (ev) => {
    ev.preventDefault();
    if (ev.submitter?.value !== "salvar") return descartar();
    erroEl.hidden = true;
    limparErros(form);
    const erros = validarCampos(form);
    if (erros.length) return mostrarErros(erros, erroEl);
    const botao = ev.submitter;
    botao.disabled = true;
    try {
      // `salvar` pode devolver o que fazer depois (recarregar a lista): roda com o diálogo já
      // fechado, para uma falha ali não parecer falha da ação e levar a repeti-la.
      const depois = await salvar(lerForm(form), form);
      dlg.close();
      if (typeof depois === "function") Promise.resolve().then(depois).catch((e) => toast(String(e), true));
    } catch (e) {
      if (e instanceof ErroCampo && form.elements[e.campo]) {
        mostrarErros([{ el: form.elements[e.campo], msg: e.message }], erroEl);
      } else {
        mostrarErro(erroEl, e);
        erroEl.scrollIntoView({ block: "nearest" });
      }
    } finally {
      botao.disabled = false;
    }
  };
  dlg.oncancel = (ev) => {
    ev.preventDefault(); // o Esc passa pela mesma pergunta do botão Cancelar
    descartar();
  };
  dlg.showModal();
  const corpo = $(".dialogo-corpo", dlg);
  if (corpo) corpo.scrollTop = 0;
  focarPrimeiroCampo(form);
}

// Enter salva o diálogo (Ctrl+Enter também dentro de textos longos); no campo de CNPJ, Enter consulta.
// Sem isto, o Enter do formulário acionaria o primeiro botão, que é o "Cancelar".
document.addEventListener("keydown", (ev) => {
  if (ev.key !== "Enter" || ev.defaultPrevented || ev.shiftKey || ev.altKey) return;
  const el = ev.target;
  const form = el.closest?.("dialog form");
  if (!form || el.tagName === "BUTTON" || el.type === "file") return;
  if (el.tagName === "TEXTAREA" && !ev.ctrlKey) return;
  ev.preventDefault();
  const consultar = !ev.ctrlKey && el.closest(".com-botao")?.querySelector("[data-consultar-cnpj]");
  if (consultar) return consultar.click();
  form.requestSubmit(form.querySelector('button[value="salvar"]'));
});

// Campos obrigatórios: asterisco na legenda (o leitor de tela já anuncia "obrigatório" pelo atributo).
for (const el of $$("[required]")) {
  if (el.id) document.querySelector(`label[for="${el.id}"]`)?.classList.add("obrigatorio");
}

// ---------------------------------------------------------------- trava (senha mestra)

let cofreExiste = true;

// Descarta a chave da memória e recarrega a interface: volta à tela de senha, sem dados na tela.
$("#bloquear").addEventListener("click", async () => {
  try {
    await invoke("bloquear_cofre");
    location.reload();
  } catch (e) {
    toast(String(e), true);
  }
});

async function iniciarTrava() {
  const s = await invoke("situacao_cofre");
  if (s.aberto) return entrar([]);
  cofreExiste = s.existe;
  if (!cofreExiste) {
    $("#trava-ajuda").textContent =
      "Primeiro uso: crie a senha mestra (mínimo 10 caracteres; uma frase é mais fácil de lembrar).";
    $("#bloco-confirmacao").hidden = false;
    $("#senha2").required = true;
    $("#senha").autocomplete = "new-password";
    $("#botao-trava").textContent = "Criar e abrir";
  }
  $("#senha").focus();
}

$("#form-trava").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const erro = $("#erro-trava");
  erro.hidden = true;
  const senha = $("#senha").value;
  if (!cofreExiste) {
    if ([...senha].length < 10) return mostrarErro(erro, "A senha mestra precisa ter pelo menos 10 caracteres.");
    if (senha !== $("#senha2").value) return mostrarErro(erro, "As senhas não conferem.");
  }
  const botao = $("#botao-trava");
  botao.disabled = true;
  botao.textContent = "Abrindo...";
  try {
    const abertura = await chamar(cofreExiste ? "abrir_cofre" : "criar_cofre", { senha });
    $("#senha").value = "";
    $("#senha2").value = "";
    await entrar(abertura.pendencias);
  } catch (e) {
    mostrarErro(erro, e);
    botao.disabled = false;
    botao.textContent = cofreExiste ? "Abrir" : "Criar e abrir";
  }
});

async function entrar(pendencias) {
  $("#trava").hidden = true;
  $("#app").hidden = false;
  const alertas = $("#alertas");
  alertas.innerHTML = "";
  if (pendencias.length) {
    alertas.innerHTML = `<div class="aviso atencao"><strong>Envios anteriores verificados na Sefin:</strong>
      <ul>${pendencias.map((p) => `<li>${esc(p)}</li>`).join("")}</ul></div>`;
  }
  $("#mes-ref").value = mesAnterior();
  $("#mes-notas").value = mesLembrado("mes-notas", mesAtual());
  $("#mes-lote").value = mesLembrado("mes-lote", mesAnterior());
  $("#mes-planilha").value = mesAnterior();
  await carregarEmpresas();
  await avisarVencimentos();
  procurarAtualizacao(true);
  if (!estado.empresas.length) {
    irPara("empresas");
    toast("Cadastre a primeira empresa para começar.");
    return;
  }
  const ultima = lerPreferencia("tela");
  if (ultima && ultima !== "emitir" && TELAS.includes(ultima)) irPara(ultima);
}

async function avisarVencimentos() {
  const lista = await invoke("certificados_vencendo", { dias: 30 }).catch(() => []);
  if (!lista.length) return;
  const itens = lista
    .map((v) => `<li>${esc(v.empresa)}: ${v.dias < 0 ? "vencido" : `vence em ${v.dias} dia(s)`} (${esc(v.valido_ate)})</li>`)
    .join("");
  $("#alertas").insertAdjacentHTML(
    "beforeend",
    `<div class="aviso atencao"><strong>Certificados vencendo:</strong><ul>${itens}</ul></div>`,
  );
}

// ---------------------------------------------------------------- navegação e empresa atual

/** Mostra uma tela; `focar` leva o foco ao título dela (troca feita pelo operador no menu). */
function irPara(tela, focar = false) {
  for (const b of $$("nav button[data-tela]")) {
    const ativo = b.dataset.tela === tela;
    b.classList.toggle("ativo", ativo);
    if (ativo) b.setAttribute("aria-current", "page");
    else b.removeAttribute("aria-current");
  }
  for (const s of $$("section[data-tela]")) s.hidden = s.dataset.tela !== tela;
  if (focar) $(`section[data-tela="${tela}"] h1`)?.focus();
  gravarPreferencia("tela", tela);
  const carregar = {
    notas: carregarNotas,
    lote: carregarLote,
    recorrentes: carregarRecorrencias,
    planilha: async () => desenharPlanilha(),
    tomadores: carregarTomadores,
    servicos: carregarServicos,
    empresas: renderizarEmpresas,
    configuracoes: carregarConfiguracoes,
  }[tela];
  carregar?.().catch((e) => toast(String(e), true));
}

for (const b of $$("nav button[data-tela]")) b.addEventListener("click", () => irPara(b.dataset.tela, true));

// ---------------------------------------------------------------- menu lateral recolhível

/** Abaixo desta largura (em rem, então já conta o tamanho do texto escolhido) o menu recolhe sozinho. */
const LARGURA_RECOLHE_MENU = 74;

const menuRecolheSozinho = () =>
  innerWidth / parseFloat(getComputedStyle(document.documentElement).fontSize) < LARGURA_RECOLHE_MENU;

/** Preferência "auto" (pela largura), "expandida" ou "recolhida"; guardada na máquina. */
function atualizarMenu() {
  const pref = lerPreferencia("menu") ?? "auto";
  const recolhido = pref === "recolhida" || (pref === "auto" && menuRecolheSozinho());
  $("#app").classList.toggle("menu-recolhido", recolhido);
  const b = $("#alternar-nav");
  b.setAttribute("aria-expanded", String(!recolhido));
  b.textContent = recolhido ? "»" : "« Recolher menu";
  b.setAttribute("aria-label", recolhido ? "Expandir menu" : "Recolher menu");
  b.title = recolhido ? "Expandir menu" : "Recolher menu";
}

$("#alternar-nav").addEventListener("click", () => {
  const recolher = !$("#app").classList.contains("menu-recolhido");
  // se o operador escolhe o mesmo que a largura escolheria, volta ao automático
  gravarPreferencia("menu", recolher === menuRecolheSozinho() ? "auto" : recolher ? "recolhida" : "expandida");
  atualizarMenu();
});
addEventListener("resize", atualizarMenu);
atualizarMenu();

async function carregarEmpresas() {
  estado.empresas = await invoke("listar_empresas");
  const sel = $("#empresa-atual");
  const anterior = estado.empresa?.id ?? Number(lerPreferencia("empresa"));
  sel.innerHTML = estado.empresas.length
    ? estado.empresas.map((e) => `<option value="${e.id}">${esc(e.nome)} — ${esc(e.cnpj_formatado)}</option>`).join("")
    : `<option value="">Nenhuma empresa cadastrada</option>`;
  const escolhida = estado.empresas.find((e) => e.id === anterior) ?? estado.empresas[0] ?? null;
  if (escolhida) sel.value = String(escolhida.id);
  await trocarEmpresa(escolhida);
}

function lerPreferencia(chave) {
  try {
    return localStorage.getItem("emissor." + chave);
  } catch {
    return null;
  }
}

function gravarPreferencia(chave, valor) {
  try {
    localStorage.setItem("emissor." + chave, valor);
  } catch {
    /* sem armazenamento local: apenas não lembra */
  }
}

$("#empresa-atual").addEventListener("change", (ev) => {
  const e = estado.empresas.find((x) => x.id === Number(ev.target.value)) ?? null;
  trocarEmpresa(e).catch((err) => toast(String(err), true));
});

async function trocarEmpresa(empresa) {
  if (empresa?.id !== estado.empresa?.id) descartarRevisao();
  estado.empresa = empresa;
  const selo = $("#selo-ambiente");
  const info = $("#info-certificado");
  if (!empresa) {
    selo.hidden = true;
    info.textContent = "";
    estado.servicos = [];
    renderizarServicosEmitir();
    return;
  }
  gravarPreferencia("empresa", empresa.id);
  selo.hidden = false;
  selo.className = "selo " + empresa.ambiente;
  selo.textContent = NOME_AMBIENTE[empresa.ambiente];
  // validade curta no cabeçalho; carimbo quando pede atenção; o texto completo fica no title
  const c = empresa.certificado;
  const d = c?.dias_para_vencer;
  if (!c) {
    info.className = "selo rejeitada";
    info.textContent = "Sem certificado";
    info.title = "Importe o certificado A1 da empresa em Empresas.";
  } else if (d < 0) {
    info.className = "selo rejeitada";
    info.textContent = "Cert. vencido";
    info.title = `O certificado venceu em ${c.valido_ate}.`;
  } else if (d <= 30) {
    info.className = "selo incerta";
    info.textContent = `Cert. ${d} ${d === 1 ? "dia" : "dias"}`;
    info.title = `Certificado válido até ${c.valido_ate} — faltam ${d} ${d === 1 ? "dia" : "dias"}.`;
  } else {
    info.className = "cert-ok dado";
    info.textContent = `cert. até ${c.valido_ate}`;
    info.title = `Certificado válido até ${c.valido_ate}.`;
  }
  limparEmissao();
  estado.servicos = await invoke("listar_servicos", { empresaId: empresa.id });
  renderizarServicosEmitir();
  // Recarrega a tela visível com a nova empresa.
  const visivel = $$("section[data-tela]").find((s) => !s.hidden)?.dataset.tela;
  if (visivel && visivel !== "emitir" && visivel !== "empresas") irPara(visivel);
}

// ---------------------------------------------------------------- emitir

function renderizarServicosEmitir() {
  const sel = $("#servico");
  sel.innerHTML = estado.servicos.length
    ? estado.servicos.map((s) => `<option value="${s.id}">${esc(s.nome)}</option>`).join("")
    : `<option value="">Cadastre um serviço em "Serviços"</option>`;
  aplicarServico();
}

function aplicarServico() {
  const s = estado.servicos.find((x) => x.id === Number($("#servico").value));
  if (!s) return;
  $("#descricao").value = s.descricao;
  if (s.valor_sugerido) $("#valor").value = moeda(s.valor_sugerido);
  $("#bloco-obra").hidden = !CODIGOS_COM_OBRA.includes(s.codigo_trib_nac);
}

$("#servico").addEventListener("change", aplicarServico);

// ---- dados da obra (construção civil): mesma lista do nfse-core (regra E0370 da Sefin)

const CODIGOS_COM_OBRA = [
  "070201", "070202", "070401", "070501", "070502", "070601", "070602", "070701", "070801", "071701",
  "071901", "141403", "141404",
];

// O bloco da obra existe na emissão (ids `obra-*`) e no diálogo da recorrência (`rec-obra-*`).

function ajustarTipoObra(p = "obra") {
  const tipo = $(`#${p}-tipo`).value;
  $(`#${p}-campo-codigo`).hidden = tipo === "endereco";
  $(`#${p}-endereco`).hidden = tipo !== "endereco";
  $(`#${p}-codigo-rotulo`).textContent = tipo === "cib" ? "Código CIB (8 caracteres)" : "Número do CNO / CEI";
  $(`#${p}-codigo`).maxLength = tipo === "cib" ? 8 : 30;
}

for (const p of ["obra", "rec-obra"]) $(`#${p}-tipo`).addEventListener("change", () => ajustarTipoObra(p));

function limparObra(p = "obra") {
  for (const el of $$(`#bloco-${p} input`)) el.value = "";
  $(`#${p}-cep-cidade`).textContent = "";
  $(`#${p}-tipo`).value = "cno";
  ajustarTipoObra(p);
}

/** Grupo `obra` do pedido (`nfse_core::modelo::Obra` em JSON), ou null se o serviço não pede. */
function obraDigitada(p = "obra") {
  if ($(`#bloco-${p}`).hidden) return null;
  const valor = (campo) => $(`#${p}-${campo}`).value;
  const tipo = valor("tipo");
  const identificacao =
    tipo === "endereco"
      ? {
          endereco: {
            cep: soDigitos(valor("cep")),
            logradouro: valor("logradouro").trim(),
            numero: valor("numero").trim(),
            complemento: vazio(valor("complemento")),
            bairro: valor("bairro").trim(),
          },
        }
      : { [tipo]: valor("codigo").trim() };
  return { inscricao_imobiliaria: vazio(valor("inscricao")), identificacao };
}

function preencherObra(obra, p = "obra") {
  limparObra(p);
  if (!obra) return;
  $(`#${p}-inscricao`).value = obra.inscricao_imobiliaria ?? "";
  const [tipo, dado] = Object.entries(obra.identificacao)[0];
  $(`#${p}-tipo`).value = tipo;
  ajustarTipoObra(p);
  if (tipo === "endereco") {
    $(`#${p}-cep`).value = FORMATOS.cep.mascara(dado.cep);
    $(`#${p}-logradouro`).value = dado.logradouro;
    $(`#${p}-numero`).value = dado.numero;
    $(`#${p}-complemento`).value = dado.complemento ?? "";
    $(`#${p}-bairro`).value = dado.bairro;
  } else {
    $(`#${p}-codigo`).value = dado;
  }
}

function descreverObra(obra) {
  const [tipo, dado] = Object.entries(obra.identificacao)[0];
  const id =
    tipo === "endereco"
      ? `${dado.logradouro}, ${dado.numero}${dado.complemento ? " " + dado.complemento : ""} — ${dado.bairro}, CEP ${FORMATOS.cep.mascara(dado.cep)}`
      : `${tipo === "cib" ? "CIB" : "CNO/CEI"} ${dado}`;
  return obra.inscricao_imobiliaria ? `${id} (inscr. imob. ${obra.inscricao_imobiliaria})` : id;
}

function limparEmissao() {
  escolherTomador(null);
  $("#valor").value = "";
  $("#info-compl").value = "";
  limparObra();
  $("#mes-ref").value = mesAnterior();
  $("#revisao").hidden = true;
  $("#resultado").hidden = true;
}

/**
 * Alterna entre a busca de tomador e o cartão do escolhido (com "Trocar"). `texto` vazio volta para a
 * busca. Serve à emissão e ao diálogo de recorrência.
 */
function mostrarTomadorEscolhido({ escolhido, busca, resultados, termo, rotulo }, texto) {
  escolhido.hidden = !texto;
  busca.hidden = !!texto;
  resultados.hidden = true;
  termo.value = "";
  if (texto) rotulo.textContent = texto;
}

function escolherTomador(t) {
  if (t !== estado.tomador) descartarRevisao();
  estado.tomador = t;
  mostrarTomadorEscolhido(
    {
      escolhido: $("#tomador-escolhido"),
      busca: $("#busca-tomador"),
      resultados: $("#resultados-tomador"),
      termo: $("#termo-tomador"),
      rotulo: $("#tomador-escolhido-texto"),
    },
    t && `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`,
  );
}

$("#trocar-tomador").addEventListener("click", () => {
  escolherTomador(null);
  $("#termo-tomador").focus();
});

/** Vincula à empresa escolhida um tomador cadastrado por outra; e-mails e retenção começam vazios. */
async function vincularTomador(t) {
  const vinculado = await invoke("vincular_tomador", { empresaId: estado.empresa.id, pessoaId: t.pessoa_id });
  toast("Tomador vinculado a esta empresa. Confira e-mails e retenções em Tomadores.");
  return vinculado;
}

/**
 * Campo de busca no padrão combobox (ARIA 1.2): o foco fica no campo e a opção ativa é indicada
 * por aria-activedescendant; setas, Home/End, Enter, Esc e clique. `buscar(termo)` devolve a lista
 * (ou null para fechar), `opcao(item)` o HTML de cada opção e `aoEscolher(item)` recebe o escolhido.
 */
function ligarCombobox(input, caixa, { rotulo, vazio, contagem, buscar, opcao, aoEscolher }) {
  let pendente = 0;
  let sequencia = 0;
  let resultados = [];
  let foco = -1;
  caixa.id ||= `${input.id}-lista`;
  caixa.setAttribute("role", "listbox");
  caixa.setAttribute("aria-label", `${rotulo} encontrados`);
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-controls", caixa.id);
  input.setAttribute("aria-expanded", "false");

  // a lista também é escondida por quem escolhe/troca o item: aria-expanded acompanha o atributo hidden
  new MutationObserver(() => {
    input.setAttribute("aria-expanded", String(!caixa.hidden));
    if (caixa.hidden) input.removeAttribute("aria-activedescendant");
  }).observe(caixa, { attributes: true, attributeFilter: ["hidden"] });

  function desenhar() {
    caixa.hidden = false;
    caixa.innerHTML = resultados.length
      ? resultados
          .map(
            (item, i) => `<div role="option" id="${caixa.id}-${i}" data-i="${i}" aria-selected="${i === foco}"
              class="${i === foco ? "foco" : ""}">${opcao(item)}</div>`,
          )
          .join("")
      : `<div class="vazio" role="option" aria-disabled="true" aria-selected="false">${esc(vazio)}</div>`;
    if (foco >= 0) {
      input.setAttribute("aria-activedescendant", `${caixa.id}-${foco}`);
      document.getElementById(`${caixa.id}-${foco}`)?.scrollIntoView({ block: "nearest" });
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  input.addEventListener("input", () => {
    clearTimeout(pendente);
    // resposta de uma busca antiga que chega depois da nova é descartada
    const minha = ++sequencia;
    const termo = input.value.trim();
    if (!termo) {
      caixa.hidden = true;
      return;
    }
    pendente = setTimeout(async () => {
      let achados;
      try {
        achados = await buscar(termo);
      } catch (e) {
        if (minha === sequencia) toast(String(e), true);
        return;
      }
      if (minha !== sequencia) return;
      if (!achados) {
        caixa.hidden = true;
        return;
      }
      resultados = achados;
      foco = resultados.length ? 0 : -1;
      desenhar();
      anunciar(resultados.length ? contagem(resultados.length) : vazio);
    }, 200);
  });

  input.addEventListener("keydown", (ev) => {
    if (!resultados.length) return;
    if (caixa.hidden) {
      // seta para baixo reabre a última lista
      if (ev.key === "ArrowDown" && input.value.trim()) {
        ev.preventDefault();
        desenhar();
      }
      return;
    }
    const destino = { ArrowDown: foco + 1, ArrowUp: foco - 1, Home: 0, End: resultados.length - 1 }[ev.key];
    if (destino !== undefined) {
      ev.preventDefault();
      foco = (destino + resultados.length) % resultados.length;
      desenhar();
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      if (foco >= 0) aoEscolher(resultados[foco]);
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      caixa.hidden = true;
    }
  });

  // mousedown com preventDefault: o clique escolhe sem tirar o foco do campo antes da hora
  caixa.addEventListener("mousedown", (ev) => {
    ev.preventDefault();
    const item = ev.target.closest("[data-i]");
    if (item) aoEscolher(resultados[Number(item.dataset.i)]);
  });

  input.addEventListener("blur", () => {
    caixa.hidden = true;
  });
}

/**
 * Busca de tomador (emissão e diálogo de recorrência), com os de outras empresas no fim.
 * `aoEscolher(tomadorDto)` recebe o escolhido.
 */
function ligarBuscaTomador(input, caixa, aoEscolher) {
  ligarCombobox(input, caixa, {
    rotulo: "Tomadores",
    vazio: "Nenhum tomador encontrado.",
    contagem: (n) => `${n} tomador(es) encontrado(s).`,
    async buscar(termo) {
      if (!estado.empresa) return null;
      const resultados = await invoke("buscar_tomadores", { empresaId: estado.empresa.id, termo });
      const outros = await invoke("buscar_tomadores_de_outras_empresas", { empresaId: estado.empresa.id, termo });
      return [...resultados, ...outros.map((t) => ({ ...t, de_outra_empresa: true }))];
    },
    opcao: (t) => `${esc(t.tomador.nome)}
              <small>${esc(formatarDocumento(t.tomador.documento))}${t.de_outra_empresa ? " · de outra empresa: vincular" : ""}</small>`,
    // tomador de outra empresa: vincula a esta (sem cadastrar de novo) e só então escolhe
    async aoEscolher(t) {
      if (t.de_outra_empresa) {
        try {
          t = await vincularTomador(t);
        } catch (e) {
          return toast(String(e), true);
        }
      }
      aoEscolher(t);
    },
  });
}

/** Anuncia um texto curto no leitor de tela sem mostrar nada (região viva escondida). */
function anunciar(texto) {
  const el = $("#anuncio");
  el.textContent = "";
  setTimeout(() => (el.textContent = texto), 50);
}

ligarBuscaTomador($("#termo-tomador"), $("#resultados-tomador"), (t) => escolherTomador(t));

$("#novo-tomador-emitir").addEventListener("click", (ev) => {
  ev.preventDefault();
  editarTomador(null, (salvo) => escolherTomador(salvo));
});

function montarPedido() {
  if (!estado.empresa) throw "Escolha a empresa.";
  const servicoId = Number($("#servico").value);
  if (!servicoId) throw new ErroCampo("servico", 'Escolha o serviço (cadastre um em "Serviços").');
  const valor = valorDigitado($("#valor").value);
  if (!valor) throw new ErroCampo("valor", "Informe o valor.");
  return {
    empresa_id: estado.empresa.id,
    tomador: estado.tomador?.pessoa_id ?? null,
    servico_id: servicoId,
    valor,
    descricao: vazio($("#descricao").value),
    mes_referencia: vazio($("#mes-ref").value),
    obra: obraDigitada(),
    informacoes_complementares: vazio($("#info-compl").value),
    reter_federais: estado.reter?.federais ?? null,
    reter_iss: estado.reter?.iss ?? null,
  };
}

/**
 * A revisão mostra o que será emitido (`estado.pedido`); se o formulário muda depois, ela deixa de
 * valer e fecha, para o "Emitir nota" (ou o Ctrl+Enter) não mandar o pedido antigo.
 */
let emissaoEmCurso = false;

function descartarRevisao() {
  // Durante o envio a revisão fica: o pedido já foi capturado e a resposta aparece nela.
  if (emissaoEmCurso) return;
  // revisão pedida e ainda sem resposta: a resposta, com os dados antigos, não abre mais
  if ($("#revisar").disabled) {
    revisaoAtual++;
    $("#revisar").disabled = false;
  }
  if ($("#revisao").hidden) return;
  $("#revisao").hidden = true;
  estado.pedido = null;
  anunciar("Os dados mudaram: revise a nota de novo antes de emitir.");
}

for (const evento of ["input", "change"]) {
  $('section[data-tela="emitir"] .cartao').addEventListener(evento, descartarRevisao);
}

$("#revisar").addEventListener("click", () => {
  estado.reter = null; // volta ao que o cadastro do tomador diz
  return revisar();
});

/** Cada revisão ganha um número; a resposta de uma revisão mais antiga que a última é ignorada. */
let revisaoAtual = 0;

async function revisar() {
  $("#resultado").hidden = true;
  const formulario = $('section[data-tela="emitir"] .cartao');
  limparErros(formulario);
  const erros = validarCampos(formulario);
  if (erros.length) return mostrarErros(erros);
  let pedido;
  try {
    pedido = montarPedido();
  } catch (e) {
    if (e instanceof ErroCampo) return mostrarErros([{ el: $(`#${e.campo}`), msg: e.message }]);
    return toast(String(e?.message ?? e), true);
  }
  const minha = ++revisaoAtual;
  const botao = $("#revisar");
  botao.disabled = true;
  let previa;
  try {
    previa = await chamar("previa_dps", { pedido });
  } catch (e) {
    if (minha === revisaoAtual) toast(String(e), true);
    return;
  } finally {
    if (minha === revisaoAtual) botao.disabled = false;
  }
  if (minha !== revisaoAtual) return;
  estado.pedido = pedido;
  const aviso = $("#revisao-aviso");
  const partes = [];
  if (previa.ambiente === "producao") {
    partes.push(`<div class="aviso erro"><strong>PRODUÇÃO:</strong> a nota terá valor fiscal.</div>`);
  } else {
    partes.push(`<div class="aviso atencao">Homologação: nota de teste, sem valor fiscal.</div>`);
  }
  if (previa.erros.length) {
    partes.push(`<div class="aviso erro"><strong>Corrija antes de emitir:</strong>
      <ul>${previa.erros.map((e) => `<li>${esc(e)}</li>`).join("")}</ul></div>`);
  }
  for (const texto of previa.avisos) partes.push(`<div class="aviso atencao">${esc(texto)}</div>`);
  aviso.innerHTML = partes.join("");
  const linhas = [
    ["Prestador", previa.prestador],
    ["Tomador", previa.tomador ?? "Não identificado"],
    ["Valor", "R$ " + moeda(previa.valor)],
    ["Competência", previa.competencia],
    ["Série da DPS", previa.serie],
    ["Descrição", previa.descricao],
  ];
  if (pedido.obra) linhas.push(["Obra", descreverObra(pedido.obra)]);
  if (pedido.informacoes_complementares) linhas.push(["Inf. complementares", pedido.informacoes_complementares]);
  if (previa.retencao_federal) {
    linhas.push(["Retenções federais", "R$ " + moeda(previa.retencao_federal)]);
    linhas.push(["Valor líquido", "R$ " + moeda(previa.valor_liquido) + (previa.iss_retido ? " (antes do ISS retido)" : "")]);
  }
  if (previa.iss_retido) linhas.push(["ISS", "Retido pelo tomador"]);
  $("#revisao-dados").innerHTML = linhas.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("");
  const reter = $("#revisao-reter");
  reter.hidden = !previa.retencao_condicional;
  $("#reter-federais").checked = previa.reter_federais;
  $("#reter-iss").checked = previa.reter_iss;
  $("#confirmar-emissao").disabled = previa.erros.length > 0;
  $("#revisao").hidden = false;
  $("#revisao").scrollIntoView({ behavior: "smooth" });
}

/** "Reter nesta nota": refaz a prévia com a escolha, que vale mais que o cadastro do tomador. */
for (const id of ["#reter-federais", "#reter-iss"]) {
  $(id).addEventListener("change", () => {
    estado.reter = { federais: $("#reter-federais").checked, iss: $("#reter-iss").checked };
    revisar();
  });
}

$("#voltar-edicao").addEventListener("click", () => {
  $("#revisao").hidden = true;
  $("#valor").focus();
});

$("#confirmar-emissao").addEventListener("click", async () => {
  // O pedido revisado; mudar o formulário durante o envio não troca o que já foi enviado.
  const pedido = estado.pedido;
  if (!pedido) return;
  if (atualizando) return toast("A atualização do sistema está em andamento; emita depois que ele abrir de novo.", true);
  emissaoEmCurso = true;
  const botao = $("#confirmar-emissao");
  botao.disabled = true;
  botao.textContent = "Enviando à Sefin...";
  const res = $("#resultado");
  try {
    const r = await chamar("emitir", { pedido });
    $("#revisao").hidden = true;
    if (r.autorizada) {
      res.innerHTML = `<div class="aviso ok"><strong>NFS-e ${esc(r.numero ?? "")} autorizada.</strong><br>
          Chave: <code>${esc(r.chave)}</code></div>
        ${r.avisos
          .map((a) => `<div class="aviso ${a.startsWith("E-mail enviado") ? "ok" : "atencao"}">${esc(a)}</div>`)
          .join("")}
        <div class="acoes inicio">
          <button class="botao primario" data-abrir="pdf">Abrir PDF</button>
          <button class="botao" data-abrir="pasta">Abrir pasta</button>
          ${r.avisos.some((a) => a.startsWith("E-mail enviado")) ? "" : `<button class="botao" data-email>Enviar por e-mail</button>`}
          ${estado.tomador ? `<button class="botao" data-outra title="Mantém o tomador e o serviço; só falta o valor">Emitir outra para este tomador</button>` : ""}
          <button class="botao" data-recorrente title="Cria uma recorrência com este tomador, serviço, valor e descrição">Tornar recorrente</button>
        </div>`;
      res.querySelector('[data-abrir="pdf"]').onclick = () => abrirNota(r.nfse_id, false);
      res.querySelector('[data-abrir="pasta"]').onclick = () => abrirNota(r.nfse_id, true);
      const emitido = pedido;
      const tomadorEmitido = estado.tomador;
      const nota = {
        nfse_id: r.nfse_id,
        numero_nfse: r.numero,
        tomador: tomadorEmitido?.tomador.nome ?? null,
        situacao: "autorizada",
      };
      const botaoEmail = res.querySelector("[data-email]");
      if (botaoEmail) botaoEmail.onclick = () => enviarEmailNota(nota);
      const botaoOutra = res.querySelector("[data-outra]");
      if (botaoOutra) {
        botaoOutra.onclick = () => {
          escolherTomador(tomadorEmitido);
          $("#servico").value = String(emitido.servico_id);
          aplicarServico();
          res.hidden = true;
          $("#valor").select();
          $("#valor").focus();
        };
      }
      res.querySelector("[data-recorrente]").onclick = () =>
        editarRecorrencia(null, {
          tomador_id: tomadorEmitido?.pessoa_id,
          tomador: tomadorEmitido
            ? `${tomadorEmitido.tomador.nome} — ${formatarDocumento(tomadorEmitido.tomador.documento)}`
            : null,
          servico_id: emitido.servico_id,
          valor: emitido.valor.replace(",", "."),
          descricao: emitido.descricao,
          informacoes_complementares: emitido.informacoes_complementares,
          obra: emitido.obra,
        });
      // Prepara a próxima nota (mesmo serviço e mês) sem permitir reenviar a anterior por engano.
      escolherTomador(null);
      $("#info-compl").value = "";
      limparObra();
      aplicarServico();
      toast(`NFS-e ${r.numero ?? ""} autorizada.`);
    } else {
      res.innerHTML = `<div class="aviso atencao"><strong>Sem resposta definitiva da Sefin.</strong><br>${esc(r.motivo)}
        <br>Confira em "Notas emitidas" (botão Verificar) antes de emitir de novo.</div>`;
    }
  } catch (e) {
    res.innerHTML = `<div class="aviso erro"><strong>Nota não emitida.</strong><br>${esc(e)}</div>`;
  } finally {
    emissaoEmCurso = false;
    res.hidden = false;
    botao.disabled = false;
    botao.textContent = "Emitir nota";
  }
});

async function abrirNota(nfseId, pasta) {
  try {
    await invoke("abrir_nota", { nfseId, pasta });
  } catch (e) {
    toast(String(e), true);
  }
}

// ---------------------------------------------------------------- notas emitidas

let notasCarregadas = [];

async function carregarNotas() {
  const corpo = $("#lista-notas");
  $("#rodape-notas").innerHTML = "";
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(8, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  notasCarregadas = await invoke("listar_notas", { empresaId: estado.empresa.id, mes: vazio($("#mes-notas").value) });
  if (!notasCarregadas.length) {
    corpo.innerHTML = linhaVazia(8, "Nenhuma nota neste período.", { ir: "emitir", texto: "Emitir nota" });
    return;
  }
  corpo.innerHTML = notasCarregadas
    .map((n, i) => {
      // a ação mais usada em cada situação fica visível; as outras vão para "Mais"
      const acoes = [];
      if (n.situacao === "rejeitada") acoes.push({ acao: "refazer", texto: "Corrigir e emitir" });
      if (n.situacao === "incerta" || n.situacao === "enviando") acoes.push({ acao: "verificar", texto: "Verificar" });
      if (n.nfse_id) {
        acoes.push({ acao: "pdf", texto: "PDF" }, { acao: "pasta", texto: "Pasta" });
        if (n.situacao === "autorizada" || n.situacao === "cancelada") {
          acoes.push({ acao: "email", texto: "E-mail" }, { acao: "copiar", texto: "Copiar" });
        }
        if (n.situacao === "autorizada") {
          acoes.push({ acao: "conferir", texto: "Conferir na Sefin" });
          acoes.push({ acao: "cancelar", texto: "Cancelar nota", perigo: true });
        }
      }
      const alvo = n.numero_nfse ? `nota ${n.numero_nfse}` : `DPS ${n.serie}/${n.numero_dps}`;
      const motivo = n.erro && n.situacao !== "autorizada" ? esc(n.erro) : "";
      const det = detalheLinha({
        id: `det-nota-${i}`,
        colunas: 8,
        alvo,
        itens: [
          ["Tomador", esc(n.tomador ?? "Não identificado")],
          ["Descrição", esc(n.descricao ?? "")],
          ["DPS", `<span class="dado">${esc(n.serie)}/${n.numero_dps}</span>`],
          ["E-mail", n.email_enviado_em ? `enviado em ${esc(dataHora(n.email_enviado_em))}` : ""],
        ],
        motivo,
        erro: n.situacao === "rejeitada",
      });
      const email = n.email_enviado_em ? `E-mail enviado em ${dataHora(n.email_enviado_em)}` : "";
      return `<tr>
        <td>${det.botao}</td>
        <td class="dado">${esc(n.numero_nfse ?? "—")}</td>
        <td class="dado sec-2">${esc(n.serie)}/${n.numero_dps}</td>
        <td title="${esc(n.tomador)}">${esc(n.tomador ?? "—")}</td>
        <td class="sec-1" title="${esc(n.descricao)}">${esc(n.descricao ?? "")}</td>
        <td class="num">${n.valor ? "R$ " + moeda(n.valor) : ""}</td>
        <td><span class="selo ${esc(n.situacao)}" title="${esc(email)}">${esc(NOME_SITUACAO[n.situacao] ?? n.situacao)}</span></td>
        <td>${acoesLinha(acoes, i, alvo)}</td>
      </tr>${det.linha}`;
    })
    .join("");
  const autorizadas = notasCarregadas.filter((n) => n.situacao === "autorizada");
  const total = somarValores(autorizadas, (n) => n.valor);
  const outras = notasCarregadas.length - autorizadas.length;
  $("#rodape-notas").innerHTML = `<tr><td colspan="8">${notasCarregadas.length} nota(s) · ${autorizadas.length}
    autorizada(s), total <span class="dado">R$ ${moeda(total)}</span>${
      outras ? ` · ${outras} em outra situação` : ""
    }</td></tr>`;
}

$("#mes-notas").addEventListener("change", () => carregarNotas().catch((e) => toast(String(e), true)));
$("#todas-notas").addEventListener("click", () => {
  $("#mes-notas").value = "";
  carregarNotas().catch((e) => toast(String(e), true));
});

// Notas canceladas fora do sistema (portal nacional) só aparecem aqui depois desta conferência.
$("#conferir-cancelamentos").addEventListener("click", async () => {
  const botao = $("#conferir-cancelamentos");
  const autorizadas = notasCarregadas.filter((n) => n.situacao === "autorizada" && n.nfse_id);
  if (!autorizadas.length) return toast("Nenhuma nota autorizada na lista para conferir.");
  botao.disabled = true;
  let canceladas = 0;
  let falhas = 0;
  let primeiraFalha = "";
  try {
    for (const [i, n] of autorizadas.entries()) {
      botao.textContent = `Conferindo ${i + 1} de ${autorizadas.length}...`;
      try {
        if (await chamar("sincronizar_cancelamento", { nfseId: n.nfse_id })) canceladas++;
      } catch (e) {
        falhas++;
        primeiraFalha ||= `nota ${n.numero_nfse ?? ""}: ${e}`;
      }
    }
  } finally {
    botao.disabled = false;
    botao.textContent = "Conferir cancelamentos";
  }
  const resumo =
    `${autorizadas.length} nota(s) conferida(s); ` +
    (canceladas ? `${canceladas} cancelada(s) na Sefin, agora registrada(s).` : "nenhuma cancelada fora do sistema.");
  toast(falhas ? `${resumo} ${falhas} sem resposta (${primeiraFalha}).` : resumo, falhas > 0);
  if (canceladas) await carregarNotas().catch((e) => toast(String(e), true));
});

$("#abrir-pasta-mes").addEventListener("click", async () => {
  const comArquivo = notasCarregadas.find((n) => n.nfse_id);
  if (comArquivo) return abrirNota(comArquivo.nfse_id, true);
  try {
    await invoke("abrir_arquivo", { caminho: await invoke("obter_pasta") });
  } catch (e) {
    toast(String(e), true);
  }
});

$("#lista-notas").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const n = notasCarregadas[Number(b.dataset.i)];
  switch (b.dataset.acao) {
    case "pdf":
      return abrirNota(n.nfse_id, false);
    case "pasta":
      return abrirNota(n.nfse_id, true);
    case "cancelar":
      return cancelarNota(n);
    case "email":
      return enviarEmailNota(n);
    case "refazer":
      return refazerNota(n);
    case "copiar":
      return copiarNota(n);
    case "conferir":
      try {
        const cancelada = await chamar("sincronizar_cancelamento", { nfseId: n.nfse_id });
        toast(cancelada ? "A nota foi cancelada na Sefin; cancelamento registrado." : "A nota continua autorizada na Sefin.");
        if (cancelada) await carregarNotas();
      } catch (e) {
        toast(String(e), true);
      }
      return;
    case "verificar":
      b.disabled = true;
      try {
        toast(await chamar("reconciliar", { dpsId: n.dps_id }));
        await carregarNotas();
      } catch (e) {
        toast(String(e), true);
        b.disabled = false;
      }
  }
});

/**
 * Preenche a tela de emissão com os dados de uma nota (`comando`: dados_para_refazer ou
 * dados_para_copiar). Com `manterDescricao`, fica o texto enviado; senão, o do modelo do serviço,
 * a não ser que o serviço não exista mais no cadastro.
 */
async function levarParaEmissao(comando, n, { manterDescricao, aviso, orientacao }) {
  let r;
  try {
    r = await chamar(comando, { dpsId: n.dps_id });
  } catch (e) {
    return toast(String(e), true);
  }
  irPara("emitir");
  limparEmissao();
  escolherTomador(r.tomador);
  const servicoAchado = r.servico_id && estado.servicos.some((s) => s.id === r.servico_id);
  if (servicoAchado) $("#servico").value = String(r.servico_id);
  aplicarServico(); // descrição do modelo do serviço e dados da obra
  $("#valor").value = moeda(r.valor);
  if (manterDescricao || !servicoAchado) $("#descricao").value = r.descricao;
  $("#info-compl").value = r.informacoes_complementares ?? "";
  preencherObra(r.obra);
  const avisos = [aviso];
  if (!servicoAchado) avisos.push("Nenhum serviço do cadastro tem o mesmo código: escolha o serviço.");
  if (n.tomador && !r.tomador) avisos.push(`O tomador "${n.tomador}" não foi encontrado no cadastro desta empresa.`);
  const res = $("#resultado");
  res.innerHTML = `<div class="aviso atencao">${avisos.map(esc).join("<br>")}
    <br>${orientacao} A nota sai com o próximo número da série.</div>`;
  res.hidden = false;
}

/** Leva os dados de uma nota rejeitada para a tela de emissão (sai com o próximo número). */
function refazerNota(n) {
  return levarParaEmissao("dados_para_refazer", n, {
    manterDescricao: true, // já com o mês de referência aplicado
    aviso: `Refazendo a DPS ${n.serie}/${n.numero_dps}, rejeitada: ${n.erro ?? ""}`,
    orientacao: "Corrija o que causou a rejeição e clique em Revisar.",
  });
}

/**
 * Leva os dados de uma nota autorizada ou cancelada para a tela de emissão. A descrição vem do
 * modelo do serviço (com o mês de referência da tela): copiar o texto antigo repetiria o mês velho.
 */
function copiarNota(n) {
  return levarParaEmissao("dados_para_copiar", n, {
    manterDescricao: false,
    aviso: `Copiando a nota ${n.numero_nfse ?? `DPS ${n.serie}/${n.numero_dps}`}.`,
    orientacao: "Confira o mês de referência e o valor e clique em Revisar.",
  });
}

/** "2026-10-16T22:07:26Z" → "16/10/2026 19:07" (horário local). */
function dataHora(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const dois = (x) => String(x).padStart(2, "0");
  return `${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()} ${dois(d.getHours())}:${dois(d.getMinutes())}`;
}

async function enviarEmailNota(n) {
  let para = [];
  try {
    para = await chamar("destinatarios_email", { nfseId: n.nfse_id });
  } catch (e) {
    return toast(String(e), true);
  }
  const dlg = $("#dlg-email");
  const form = $("form", dlg);
  form.reset();
  $("#email-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"}${
    n.situacao === "cancelada" ? " (CANCELADA: o e-mail avisa o cancelamento)" : ""
  }`;
  form.elements.para.value = para.join("; ");
  abrirDialogo(dlg, $("#erro-email"), async (d) => {
    toast(await chamar("enviar_email", { nfseId: n.nfse_id, para: d.para }));
    return carregarNotas;
  });
}

let cancelamentoEmCurso = false;

function cancelarNota(n) {
  const dlg = $("#dlg-cancelar");
  $("#cancelar-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"} — R$ ${moeda(n.valor)}`;
  $("form", dlg).reset();
  abrirDialogo(dlg, $("#erro-cancelar"), async (d) => {
    const justificativa = d.justificativa.trim();
    if (justificativa.length < 15) throw new ErroCampo("justificativa", "Escreva pelo menos 15 caracteres.");
    if (atualizando) throw "A atualização do sistema está em andamento; cancele depois que ele abrir de novo.";
    cancelamentoEmCurso = true;
    try {
      await chamar("cancelar", { nfseId: n.nfse_id, motivo: d.motivo, justificativa });
    } finally {
      cancelamentoEmCurso = false;
    }
    toast(`NFS-e ${n.numero_nfse ?? ""} cancelada.`);
    return carregarNotas;
  });
}

// ---------------------------------------------------------------- recorrentes

let recorrenciasCarregadas = [];

async function carregarRecorrencias() {
  const corpo = $("#lista-recorrencias");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(5, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  recorrenciasCarregadas = await invoke("listar_recorrencias", { empresaId: estado.empresa.id, mes: mesAnterior() });
  corpo.innerHTML = recorrenciasCarregadas.length
    ? recorrenciasCarregadas
        .map(
          (r, i) => `<tr>
          <td title="${esc(r.tomador)}">${esc(r.tomador ?? "—")}</td>
          <td class="sec-2" title="${esc(r.servico)}">${esc(r.servico)}</td>
          <td class="sec-1" title="${esc(r.descricao_no_mes)}">${esc(r.descricao ?? "(a do serviço)")}</td>
          <td class="num" title="${r.valor ? "" : "Valor do serviço"}">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}${
            r.valor ? "" : "*"
          }</td>
          <td>${acoesLinha(
            [
              { acao: "editar", texto: "Editar" },
              { acao: "remover", texto: "Remover", perigo: true },
            ],
            i,
            `recorrência de ${r.tomador ?? "nota sem tomador"}`,
          )}</td>
        </tr>`,
        )
        .join("")
    : linhaVazia(5, 'Nenhuma recorrência nesta empresa (também dá para usar "Tornar recorrente" depois de emitir).', {
        clicar: "#nova-recorrencia",
        texto: "Nova recorrência",
      });
}

$("#lista-recorrencias").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const r = recorrenciasCarregadas[Number(b.dataset.i)];
  if (b.dataset.acao === "editar") return editarRecorrencia(r);
  if (!(await confirmar(`Remover a recorrência de ${r.tomador ?? "nota sem tomador"}? As notas já emitidas não mudam.`))) return;
  try {
    await chamar("desativar_recorrencia", { id: r.id });
    await carregarRecorrencias();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#nova-recorrencia").addEventListener("click", () => editarRecorrencia(null));

const dlgRecorrencia = $("#dlg-recorrencia");
let tomadorRecorrencia = null; // { pessoa_id, nome } escolhido no diálogo

function escolherTomadorRecorrencia(t) {
  tomadorRecorrencia = t;
  mostrarTomadorEscolhido(
    {
      escolhido: $("[data-rec-escolhido]", dlgRecorrencia),
      busca: $("[data-rec-busca]", dlgRecorrencia),
      resultados: $("[data-rec-resultados]", dlgRecorrencia),
      termo: $("[data-rec-termo]", dlgRecorrencia),
      rotulo: $("[data-rec-tomador]", dlgRecorrencia),
    },
    t?.nome,
  );
}

ligarBuscaTomador($("[data-rec-termo]", dlgRecorrencia), $("[data-rec-resultados]", dlgRecorrencia), (t) =>
  escolherTomadorRecorrencia({
    pessoa_id: t.pessoa_id,
    nome: `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`,
  }),
);
$("[data-rec-trocar]", dlgRecorrencia).addEventListener("click", () => escolherTomadorRecorrencia(null));

/** Abre o diálogo de recorrência: `r` é uma existente; `base`, dados para preencher uma nova. */
function editarRecorrencia(r, base = null) {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  if (!estado.servicos.length) return toast('Cadastre um serviço antes, em "Serviços".', true);
  const form = $("form", dlgRecorrencia);
  form.reset();
  $("#titulo-recorrencia").textContent = r ? "Editar recorrência" : "Nova recorrência";
  form.elements.servico.innerHTML = estado.servicos
    .map(
      (s) =>
        `<option value="${s.id}">${esc(s.nome)}${s.valor_sugerido ? ` (R$ ${moeda(s.valor_sugerido)})` : ""}</option>`,
    )
    .join("");
  const dados = r ?? base ?? {};
  escolherTomadorRecorrencia(dados.tomador_id ? { pessoa_id: dados.tomador_id, nome: dados.tomador } : null);
  if (dados.servico_id) form.elements.servico.value = String(dados.servico_id);
  form.elements.valor.value = dados.valor ? moeda(dados.valor) : "";
  form.elements.descricao.value = dados.descricao ?? "";
  form.elements.info.value = dados.informacoes_complementares ?? "";
  preencherObra(dados.obra ?? null, "rec-obra");
  const dicas = () => {
    const s = estado.servicos.find((x) => x.id === Number(form.elements.servico.value));
    form.elements.valor.placeholder = s?.valor_sugerido ? `${moeda(s.valor_sugerido)} (do serviço)` : "obrigatório";
    form.elements.descricao.placeholder = s?.descricao ?? "";
    $("#bloco-rec-obra").hidden = !CODIGOS_COM_OBRA.includes(s?.codigo_trib_nac);
  };
  form.elements.servico.onchange = dicas;
  dicas();
  abrirDialogo(dlgRecorrencia, $("#erro-recorrencia"), async (d) => {
    await chamar("salvar_recorrencia", {
      recorrencia: {
        id: r?.id ?? null,
        empresa_id: estado.empresa.id,
        tomador: tomadorRecorrencia?.pessoa_id ?? null,
        servico_id: Number(d.servico),
        valor: valorDigitado(d.valor),
        descricao: vazio(d.descricao),
        informacoes_complementares: vazio(d.info),
        obra: obraDigitada("rec-obra"),
      },
    });
    toast("Recorrência salva.");
    if (!$('section[data-tela="recorrentes"]').hidden) return carregarRecorrencias;
  });
}

// ---------------------------------------------------------------- lote do mês

let loteCarregado = [];
let loteEmAndamento = false;

const podeEmitirNoLote = (r) => !r.erros.length && (!r.no_mes || r.no_mes.libera);
const ambienteDe = (empresaId) => estado.empresas.find((e) => e.id === empresaId)?.ambiente;
const selecaoLote = selecaoEmTabela({
  lista: "#lista-lote",
  itens: () => loteCarregado,
  botao: "#lote-emitir",
  rotulo: "Emitir selecionadas",
  todos: "#lote-todos",
  marcar: "#lote-marcar",
  emAndamento: () => loteEmAndamento,
});
const esperar = (ms) => new Promise((r) => setTimeout(r, ms));

async function carregarLote() {
  // Durante a emissão a lista mostra o andamento; recarrega ao terminar.
  if (loteEmAndamento) return;
  const mes = $("#mes-lote").value;
  if (!mes) {
    $("#lista-lote").innerHTML = `<tr><td colspan="6" class="vazio">Escolha o mês de referência.</td></tr>`;
    return;
  }
  const soEmpresa = $("#lote-so-empresa").checked && estado.empresa;
  loteCarregado = await invoke("listar_recorrencias", { empresaId: soEmpresa ? estado.empresa.id : null, mes });
  desenharLote();
}

/** Carimbo da situação no lote e o motivo (vai para o detalhe da linha). */
function situacaoLote(r) {
  const t = r.no_mes;
  if (t && !t.libera) {
    const nota = t.numero_nfse ? ` nº ${esc(t.numero_nfse)}` : "";
    return { selo: `<span class="selo ${esc(t.situacao)}">${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)}${nota}</span>` };
  }
  const anterior = t ? `<p>Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</p>` : "";
  if (r.erros.length) {
    return {
      selo: `<span class="selo rejeitada">Não pode emitir</span>`,
      motivo: `<p>${r.erros.map(esc).join("<br>")}</p>${anterior}`,
      erro: true,
    };
  }
  return { selo: `<span class="selo pendente">Pendente</span>`, motivo: anterior };
}

/** Empresa na linha do lote/planilha: o nome corta com reticências e o carimbo de homologação fica inteiro. */
function celulaEmpresa(nome, empresaId) {
  const emHomologacao = ambienteDe(empresaId) === "homologacao";
  const hom = emHomologacao ? `<span class="selo homologacao">Homologação</span>` : "";
  const titulo = emHomologacao ? `${nome} (homologação, sem valor fiscal)` : nome;
  return `<div class="celula-flex"><span class="corta" title="${esc(titulo)}">${esc(nome ?? "—")}</span>${hom}</div>`;
}

function desenharLote() {
  const corpo = $("#lista-lote");
  $("#lote-todos").checked = false;
  $("#rodape-lote").innerHTML = "";
  if (!loteCarregado.length) {
    corpo.innerHTML = linhaVazia(
      7,
      `Nenhuma recorrência cadastrada${$("#lote-so-empresa").checked ? " nesta empresa" : ""}.`,
      { ir: "recorrentes", texto: "Cadastrar recorrência" },
    );
    selecaoLote.atualizar();
    return;
  }
  corpo.innerHTML = loteCarregado
    .map((r, i) => {
      const s = situacaoLote(r);
      const alvo = `${r.empresa} para ${r.tomador ?? "nota sem tomador"}`;
      const det = detalheLinha({
        id: `det-lote-${i}`,
        colunas: 7,
        alvo,
        itens: [
          ["Empresa", esc(r.empresa)],
          ["Tomador", esc(r.tomador ?? "Não identificado")],
          ["Descrição", esc(r.descricao_no_mes)],
        ],
        motivo: s.motivo ?? "",
        erro: !!s.erro,
        aberta: !!s.erro,
      });
      return `<tr data-i="${i}">
        <td><input type="checkbox" data-i="${i}" ${podeEmitirNoLote(r) ? "" : "disabled"} aria-label="${esc(`Marcar ${alvo}`)}"></td>
        <td>${det.botao}</td>
        <td>${celulaEmpresa(r.empresa, r.empresa_id)}</td>
        <td title="${esc(r.tomador)}">${esc(r.tomador ?? "—")}</td>
        <td class="sec-1" title="${esc(r.descricao_no_mes)}">${esc(r.descricao_no_mes)}</td>
        <td class="num">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}</td>
        <td data-situacao>${s.selo}</td>
      </tr>${det.linha}`;
    })
    .join("");
  const pendentes = loteCarregado.filter(podeEmitirNoLote);
  const total = somarValores(pendentes, (r) => r.valor_efetivo);
  const comErro = loteCarregado.filter((r) => r.erros.length).length;
  $("#rodape-lote").innerHTML = `<tr><td colspan="7">${loteCarregado.length} recorrência(s) · ${pendentes.length}
    pendente(s), total <span class="dado">R$ ${moeda(total)}</span>${
      comErro ? ` · ${comErro} com erro` : ""
    }</td></tr>`;
  selecaoLote.atualizar();
}

$("#mes-lote").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));
$("#lote-so-empresa").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));

/**
 * Andamento do lote/planilha: contagem, um segmento por nota na ordem da fila e o total por
 * situação em carimbos (a barra sozinha diria só por cor). É uma progressbar para o leitor de tela.
 */
function desenharAndamento(el, total, situacoes, atual) {
  const feitas = situacoes.length;
  const conta = (s) => situacoes.filter((x) => x === s).length;
  const segmentos = Array.from({ length: total }, (_, i) => {
    const s = situacoes[i];
    const classe = s === "ok" ? "ok" : s === "erro" ? "erro" : s === "incerta" ? "incerta" : i === feitas ? "atual" : "";
    return `<span class="${classe}"></span>`;
  }).join("");
  const carimbos = [
    [conta("ok"), "autorizada", "autorizada(s)"],
    [conta("erro"), "rejeitada", "não emitida(s)"],
    [conta("incerta"), "incerta", "sem resposta"],
    [total - feitas - 1, "pendente", "na fila"],
  ]
    .filter(([n]) => n > 0)
    .map(([n, classe, texto]) => `<span class="selo ${classe}">${n} ${texto}</span>`)
    .join("");
  el.innerHTML = `<div class="cartao andamento" role="progressbar" aria-label="Andamento da emissão"
      aria-valuemin="0" aria-valuemax="${total}" aria-valuenow="${feitas}"
      aria-valuetext="${feitas} de ${total} concluídas; emitindo ${esc(atual)}">
    <div class="contagem"><span>Emitindo <b>${esc(atual)}</b>. Não feche o programa.</span>
      <span class="dado">${feitas + 1} / ${total}</span></div>
    <div class="barra${total > 60 ? " continua" : ""}" aria-hidden="true">${segmentos}</div>
    <div class="legenda-lote">${carimbos}</div>
  </div>`;
}

/**
 * Emite os itens em sequência, mostrando o andamento em `andamento` e o resultado na célula de
 * cada linha (`celulaDe(item)`). `emitirUm(item)` chama o comando; com limite de requisições da
 * Sefin, espera e tenta mais uma vez. Devolve o resumo em HTML.
 */
async function emitirEmSequencia({ itens, andamento, celulaDe, descrever, emitirUm }) {
  const contagem = { autorizada: 0, falha: 0, incerta: 0 };
  const situacoes = []; // "ok" | "erro" | "incerta", na ordem da fila
  // As células são achadas antes de começar: se a lista for redesenhada no meio, a fila continua
  // (escrevendo em células que já saíram da tela) em vez de parar com a nota em curso sem registro.
  const sobra = document.createElement("td");
  const celulas = new Map(itens.map((item) => [item, celulaDe(item) ?? sobra]));
  for (const item of itens) {
    desenharAndamento(andamento, itens.length, situacoes, descrever(item));
    const celula = celulas.get(item);
    celula.innerHTML = `<span class="selo enviando">Enviando...</span>`;
    let resultado;
    for (let tentativa = 1; !resultado; tentativa++) {
      try {
        resultado = { ok: true, res: await emitirUm(item) };
      } catch (e) {
        // só o 429 da Sefin (texto de `ErroSefin::LimiteRequisicoes`): uma rejeição que fale em
        // "limite" não é reenviada
        if (tentativa === 1 && String(e).includes("(HTTP 429)")) {
          celula.innerHTML = `<span class="selo enviando">Aguardando a Sefin...</span>`;
          await esperar(15000);
          continue;
        }
        resultado = { ok: false, erro: String(e) };
      }
    }
    // o carimbo fica na célula; avisos e motivos vão para o detalhe da linha (aberto quando não emitiu)
    const linha = celula.closest("tr");
    const motivo = (html, opcoes) => linha && definirMotivo(linha, html, opcoes);
    if (resultado.ok && resultado.res.autorizada) {
      contagem.autorizada++;
      situacoes.push("ok");
      celula.innerHTML = `<span class="selo autorizada">Autorizada nº ${esc(resultado.res.numero ?? "")}</span>`;
      motivo(resultado.res.avisos.map((a) => `<p>${esc(a)}</p>`).join(""));
    } else if (resultado.ok) {
      contagem.incerta++;
      situacoes.push("incerta");
      celula.innerHTML = `<span class="selo incerta">Sem resposta</span>`;
      motivo(`<p>${esc(resultado.res.motivo)}</p>`, { abrir: true });
      anunciar(`${descrever(item)}: sem resposta da Sefin.`);
    } else {
      contagem.falha++;
      situacoes.push("erro");
      celula.innerHTML = `<span class="selo rejeitada">Não emitida</span>`;
      motivo(`<p>${esc(resultado.erro)}</p>`, { erro: true, abrir: true });
      anunciar(`${descrever(item)}: não emitida.`);
    }
  }
  andamento.innerHTML = "";
  toast(`Concluído: ${contagem.autorizada} autorizada(s).`);
  return (
    `<div class="aviso ${contagem.falha || contagem.incerta ? "atencao" : "ok"}">Concluído: ` +
    `${contagem.autorizada} autorizada(s)` +
    (contagem.falha ? `, ${contagem.falha} não emitida(s) — o motivo aparece embaixo de cada linha` : "") +
    (contagem.incerta ? `, ${contagem.incerta} sem resposta — confira em "Notas emitidas" (Verificar)` : "") +
    ".</div>"
  );
}

/**
 * Roda a fila travando os controles que redesenham a lista (mês, filtros, reler). Erro inesperado
 * vira o resumo, e os controles e o "em andamento" sempre voltam (senão o botão ficava preso).
 */
async function emitirTravando(controles, marcarAndamento, opcoes) {
  if (atualizando) return `<div class="aviso erro">${esc("A atualização do sistema está em andamento; emita depois que ele abrir de novo.")}</div>`;
  marcarAndamento(true);
  for (const c of controles) $(c).disabled = true;
  try {
    return await emitirEmSequencia(opcoes);
  } catch (e) {
    return `<div class="aviso erro">A emissão parou: ${esc(String(e))}. Confira a situação das notas em "Notas emitidas".</div>`;
  } finally {
    for (const c of controles) $(c).disabled = false;
    marcarAndamento(false);
  }
}

/** Confirmação antes de emitir em lote: quantidade, total e quantas em Produção. */
function confirmarEmissao(quantidade, total, producao, referencia) {
  return confirmar(
    `Emitir ${quantidade} nota(s)${referencia ? ` com referência ${referencia}` : ""}, total R$ ${moeda(total)}?` +
      (producao ? `\n\n${producao} em PRODUÇÃO (valor fiscal).` : ""),
  );
}

$("#lote-emitir").addEventListener("click", async () => {
  const itens = selecaoLote.marcados();
  const mes = $("#mes-lote").value;
  if (!itens.length || !mes) return;
  const [ano, m] = mes.split("-");
  const producao = itens.filter((r) => ambienteDe(r.empresa_id) === "producao").length;
  const total = somarValores(itens, (r) => r.valor_efetivo);
  if (!(await confirmarEmissao(itens.length, total, producao, `${m}/${ano}`))) return;

  for (const c of $$("#lista-lote input[type=checkbox]")) c.disabled = true;
  const andamento = (sim) => {
    loteEmAndamento = sim;
    selecaoLote.atualizar();
  };
  const resumo = await emitirTravando(["#mes-lote", "#lote-so-empresa"], andamento, {
    itens,
    andamento: $("#lote-andamento"),
    celulaDe: (r) => $(`#lista-lote tr[data-i="${loteCarregado.indexOf(r)}"] [data-situacao]`),
    descrever: (r) => `${r.empresa} → ${r.tomador ?? "sem tomador"}`,
    emitirUm: (r) => invoke("emitir_recorrencia", { recorrenciaId: r.id, mes }),
  });
  // Recarrega para mostrar a situação gravada (as rejeitadas voltam a ficar pendentes).
  await carregarLote().catch((e) => toast(String(e), true));
  $("#lote-andamento").innerHTML = resumo;
});

// ---------------------------------------------------------------- importar planilha

let planilhaLida = [];
let planilhaArquivo = null; // { nome, bytes }
let planilhaEmAndamento = false;

const podeEmitirDaPlanilha = (it) => !it.erros.length && (!it.anterior || it.anterior.libera);
const selecaoPlanilha = selecaoEmTabela({
  lista: "#lista-planilha",
  itens: () => planilhaLida,
  botao: "#planilha-emitir",
  rotulo: "Emitir selecionadas",
  todos: "#planilha-todos",
  marcar: "#planilha-marcar",
  emAndamento: () => planilhaEmAndamento,
});

$("#modelo-planilha").addEventListener("click", async () => {
  try {
    toast(`Modelo salvo em ${await chamar("salvar_modelo_planilha")}`);
  } catch (e) {
    toast(String(e), true);
  }
});

$("#arquivo-planilha").addEventListener("change", async (ev) => {
  const arquivo = ev.target.files[0];
  if (!arquivo) return;
  planilhaArquivo = { nome: arquivo.name, bytes: Array.from(new Uint8Array(await arquivo.arrayBuffer())) };
  await lerPlanilha();
});

$("#mes-planilha").addEventListener("change", () => planilhaArquivo && lerPlanilha());
$("#reler-planilha").addEventListener("click", () => planilhaArquivo && lerPlanilha());

async function lerPlanilha() {
  // Durante a emissão a lista mostra o andamento; relê ao terminar.
  if (planilhaEmAndamento) return;
  $("#planilha-andamento").innerHTML = "";
  try {
    planilhaLida = await chamar("ler_planilha", {
      nome: planilhaArquivo.nome,
      bytes: planilhaArquivo.bytes,
      empresaId: estado.empresa?.id ?? null,
      mes: $("#mes-planilha").value,
    });
  } catch (e) {
    planilhaLida = [];
    $("#planilha-andamento").innerHTML = `<div class="aviso erro">${esc(e)}</div>`;
  }
  desenharPlanilha();
}

/** Carimbo da situação da linha da planilha e o motivo (vai para o detalhe da linha). */
function situacaoPlanilha(it) {
  const t = it.anterior;
  if (t && !t.libera) {
    const nota = t.numero_nfse ? ` nº ${esc(t.numero_nfse)}` : "";
    return {
      selo: `<span class="selo ${esc(t.situacao)}">Já ${esc((NOME_SITUACAO[t.situacao] ?? t.situacao).toLowerCase())}${nota}</span>`,
    };
  }
  if (it.erros.length) {
    const cadastrar =
      !it.pessoa_id && it.documento_tomador && it.empresa_id
        ? `<p><button type="button" class="botao pequeno" data-cadastrar-tomador="${planilhaLida.indexOf(it)}">Cadastrar tomador</button></p>`
        : "";
    return {
      selo: `<span class="selo rejeitada">Não pode emitir</span>`,
      motivo: `<p>${it.erros.map(esc).join("<br>")}</p>${cadastrar}`,
      erro: true,
    };
  }
  const aviso = it.vincular_tomador ? `<p>O tomador será vinculado a esta empresa.</p>` : "";
  const antes = t ? `<p>Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</p>` : "";
  return { selo: `<span class="selo pendente">Pronta</span>`, motivo: aviso + antes };
}

function desenharPlanilha() {
  const corpo = $("#lista-planilha");
  $("#planilha-todos").checked = false;
  if (!planilhaLida.length) {
    corpo.innerHTML = `<tr><td colspan="8" class="vazio">${
      planilhaArquivo ? "Nenhuma linha com dados na planilha." : "Escolha a planilha (.xlsx, .xls, .ods ou .csv)."
    }</td></tr>`;
    $("#planilha-resumo").textContent = "";
    selecaoPlanilha.atualizar();
    return;
  }
  corpo.innerHTML = planilhaLida
    .map((it, i) => {
      const s = situacaoPlanilha(it);
      const tomador = it.tomador ?? it.documento_tomador;
      const alvo = `linha ${it.linha}`;
      const det = detalheLinha({
        id: `det-planilha-${i}`,
        colunas: 8,
        alvo,
        itens: [
          ["Empresa", esc(it.empresa ?? "—")],
          ["Tomador", esc(tomador)],
          ["Descrição", esc(it.descricao)],
        ],
        motivo: s.motivo ?? "",
        erro: !!s.erro,
        aberta: !!s.erro,
      });
      return `<tr data-i="${i}">
        <td><input type="checkbox" data-i="${i}" ${podeEmitirDaPlanilha(it) ? "" : "disabled"} aria-label="${esc(`Marcar ${alvo}`)}"></td>
        <td>${det.botao}</td>
        <td class="num">${it.linha}</td>
        <td class="sec-2">${celulaEmpresa(it.empresa, it.empresa_id)}</td>
        <td title="${esc(tomador)}">${esc(tomador)}</td>
        <td class="sec-1" title="${esc(it.descricao)}">${esc(it.descricao)}</td>
        <td class="num">${it.valor ? "R$ " + moeda(it.valor) : ""}</td>
        <td data-situacao>${s.selo}</td>
      </tr>${det.linha}`;
    })
    .join("");
  const prontas = planilhaLida.filter(podeEmitirDaPlanilha);
  const total = somarValores(prontas, (it) => it.valor);
  const jaEmitidas = planilhaLida.filter((it) => it.anterior && !it.anterior.libera).length;
  const comErro = planilhaLida.length - prontas.length - jaEmitidas;
  $("#planilha-resumo").textContent =
    `${planilhaLida.length} linha(s): ${prontas.length} pronta(s) para emitir (R$ ${moeda(total)})` +
    (jaEmitidas ? `, ${jaEmitidas} já emitida(s)` : "") +
    (comErro ? `, ${comErro} com erro` : "") +
    ".";
  selecaoPlanilha.atualizar();
}

$("#lista-planilha").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-cadastrar-tomador]");
  if (!b) return;
  const it = planilhaLida[Number(b.dataset.cadastrarTomador)];
  editarTomador(null, () => lerPlanilha(), { empresaId: it.empresa_id, documento: it.documento_tomador });
});

$("#planilha-emitir").addEventListener("click", async () => {
  const itens = selecaoPlanilha.marcados();
  if (!itens.length) return;
  const producao = itens.filter((it) => ambienteDe(it.empresa_id) === "producao").length;
  const total = somarValores(itens, (it) => it.valor);
  if (!(await confirmarEmissao(itens.length, total, producao, null))) return;

  for (const c of $$("#lista-planilha input[type=checkbox]")) c.disabled = true;
  const andamento = (sim) => {
    planilhaEmAndamento = sim;
    selecaoPlanilha.atualizar();
  };
  const controles = ["#mes-planilha", "#reler-planilha", "#arquivo-planilha"];
  const resumo = await emitirTravando(controles, andamento, {
    itens,
    andamento: $("#planilha-andamento"),
    celulaDe: (it) => $(`#lista-planilha tr[data-i="${planilhaLida.indexOf(it)}"] [data-situacao]`),
    descrever: (it) => `linha ${it.linha}: ${it.tomador ?? it.documento_tomador}`,
    emitirUm: (it) => invoke("emitir_planilha", { item: it, arquivo: planilhaArquivo?.nome ?? null }),
  });
  await lerPlanilha();
  $("#planilha-andamento").innerHTML = resumo;
});

// ---------------------------------------------------------------- tomadores

let tomadoresCarregados = [];

async function carregarTomadores() {
  const corpo = $("#lista-tomadores");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(5, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  tomadoresCarregados = await invoke("buscar_tomadores", {
    empresaId: estado.empresa.id,
    termo: $("#busca-tomadores").value,
  });
  const termo = $("#busca-tomadores").value.trim();
  if (termo) {
    const outros = await invoke("buscar_tomadores_de_outras_empresas", { empresaId: estado.empresa.id, termo });
    tomadoresCarregados.push(...outros.map((t) => ({ ...t, de_outra_empresa: true })));
  }
  corpo.innerHTML = tomadoresCarregados.length
    ? tomadoresCarregados
        .map(
          (t, i) => `<tr>
          <td class="dado">${esc(formatarDocumento(t.tomador.documento))}</td>
          <td title="${esc(t.tomador.nome)}">${esc(t.tomador.nome)}${t.de_outra_empresa ? " (de outra empresa)" : ""}</td>
          <td class="dado sec-1">${esc(t.tomador.endereco?.municipio ?? "")}</td>
          <td class="sec-2" title="${esc(t.emails)}">${esc(t.emails ?? "")}</td>
          <td>${acoesLinha([t.de_outra_empresa ? { acao: "vincular", texto: "Vincular" } : { acao: "editar", texto: "Editar" }], i, t.tomador.nome)}</td>
        </tr>`,
        )
        .join("")
    : linhaVazia(5, "Nenhum tomador encontrado (a lista mostra até 20; use a busca).", {
        clicar: "#novo-tomador",
        texto: "Novo tomador",
      });
}

let buscaTomadores = 0;
$("#busca-tomadores").addEventListener("input", () => {
  clearTimeout(buscaTomadores);
  buscaTomadores = setTimeout(() => carregarTomadores().catch((e) => toast(String(e), true)), 250);
});

$("#lista-tomadores").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const t = tomadoresCarregados[Number(b.dataset.i)];
  if (b.dataset.acao === "vincular") {
    vincularTomador(t)
      .then(() => carregarTomadores())
      .catch((e) => toast(String(e), true));
  } else {
    editarTomador(t, () => carregarTomadores());
  }
});

$("#novo-tomador").addEventListener("click", () => editarTomador(null, () => carregarTomadores()));

/** Abre o diálogo de tomador; `depois(tomadorDto)` recebe o tomador gravado. */
function editarTomador(dto, depois, { empresaId = estado.empresa?.id, documento = null } = {}) {
  if (!empresaId) return toast("Escolha a empresa.", true);
  const dlg = $("#dlg-tomador");
  const form = $("form", dlg);
  form.reset();
  $("[data-municipio-nome]", dlg).textContent = "";
  $("[data-aviso-cnpj]", dlg).hidden = true;
  if (dto) {
    const t = dto.tomador;
    const e = t.endereco ?? {};
    preencherForm(form, {
      tipo: t.documento.cpf ? "cpf" : "cnpj",
      documento: formatarDocumento(t.documento),
      im: t.inscricao_municipal,
      nome: t.nome,
      cep: e.cep,
      municipio: e.municipio,
      logradouro: e.logradouro,
      numero: e.numero,
      complemento: e.complemento,
      bairro: e.bairro,
      telefone: t.telefone,
      email: t.email,
      emails: dto.emails,
      retem_federais: dto.retem_federais,
      retem_iss: dto.retem_iss,
    });
  }
  abrirDialogo(dlg, $("#erro-tomador"), async (d) => {
    const documento = d.tipo === "cpf" ? soDigitos(d.documento) : d.documento.replace(/[^0-9A-Za-z]/g, "").toUpperCase();
    const tomador = {
      documento: { [d.tipo]: documento },
      nome: d.nome.trim(),
    };
    if (vazio(d.im)) tomador.inscricao_municipal = vazio(d.im);
    if (vazio(d.telefone)) tomador.telefone = soDigitos(d.telefone);
    if (vazio(d.email)) tomador.email = vazio(d.email);
    const temEndereco = [d.cep, d.municipio, d.logradouro, d.numero, d.bairro].some((v) => vazio(v));
    if (temEndereco) {
      const faltando = ["cep", "municipio", "logradouro", "bairro"].find((campo) => !vazio(d[campo]));
      if (faltando) throw new ErroCampo(faltando, "Endereço começado: preencha também este campo.");
      tomador.endereco = {
        municipio: soDigitos(d.municipio),
        cep: soDigitos(d.cep),
        logradouro: d.logradouro.trim(),
        numero: vazio(d.numero) ?? "S/N",
        bairro: d.bairro.trim(),
      };
      if (vazio(d.complemento)) tomador.endereco.complemento = vazio(d.complemento);
    }
    const emails = vazio(d.emails);
    const retemFederais = !!d.retem_federais;
    const retemIss = !!d.retem_iss;
    const pessoaId = await chamar("salvar_tomador", { empresaId, tomador, emails, retemFederais, retemIss });
    toast("Tomador salvo.");
    depois?.({ pessoa_id: pessoaId, tomador, emails, retem_federais: retemFederais, retem_iss: retemIss });
  });
  if (!dto && documento) {
    // Vindo da planilha: já preenche o documento e, se for CNPJ, consulta os dados.
    const cpf = documento.length === 11;
    form.elements.tipo.value = cpf ? "cpf" : "cnpj";
    form.elements.documento.value = formatarDocumento(cpf ? { cpf: documento } : { cnpj: documento });
    form.elements.documento.dispatchEvent(new Event("change"));
  }
}

// ---------------------------------------------------------------- consulta de CNPJ

/**
 * Liga o botão "Consultar" (e a saída do campo, quando o nome ainda está vazio) à consulta
 * pública de CNPJ. `preencher(form, dados)` copia o que veio para o formulário.
 */
function ligarConsultaCnpj(dlg, campo, ehCnpj, preencher) {
  const form = $("form", dlg);
  const aviso = $("[data-aviso-cnpj]", dlg);
  const botao = $("[data-consultar-cnpj]", dlg);

  async function consultar(automatico) {
    const cnpj = form.elements[campo].value.replace(/[^0-9A-Za-z]/g, "");
    if (!ehCnpj(form)) return automatico || toast("A consulta é só para CNPJ.", true);
    if (cnpj.length !== 14) return automatico || toast("Informe os 14 caracteres do CNPJ.", true);
    aviso.hidden = true;
    botao.disabled = true;
    botao.textContent = "Consultando...";
    try {
      const d = await chamar("consultar_cnpj", { cnpj });
      preencher(form, d);
      if (d.situacao && d.situacao.toLowerCase() !== "ativa") {
        aviso.textContent = `Atenção: situação cadastral na Receita é "${d.situacao}".`;
        aviso.hidden = false;
      }
    } catch (e) {
      toast(String(e), true);
    } finally {
      botao.disabled = false;
      botao.textContent = "Consultar";
    }
  }

  botao.addEventListener("click", () => consultar(false));
  form.elements[campo].addEventListener("change", () => {
    if (vazio(form.elements.nome.value)) return; // não sobrescreve um cadastro já preenchido
    consultar(true);
  });
}

/** Copia só os valores que vieram da consulta (não apaga o que o operador já digitou). */
function copiarSeVeio(form, pares) {
  for (const [nome, valor] of pares) if (valor != null && valor !== "") form.elements[nome].value = valor;
}

ligarConsultaCnpj(
  $("#dlg-tomador"),
  "documento",
  (form) => form.elements.tipo.value === "cnpj",
  (form, d) => {
    copiarSeVeio(form, [
      ["nome", d.razao_social],
      ["cep", d.cep],
      ["municipio", d.municipio],
      ["logradouro", d.logradouro],
      ["numero", d.numero],
      ["complemento", d.complemento],
      ["bairro", d.bairro],
      ["telefone", d.telefone],
      ["email", d.email],
    ]);
    $("[data-municipio-nome]", form).textContent = d.municipio_nome ? `${d.municipio_nome}/${d.uf ?? ""}` : "";
  },
);

// ---------------------------------------------------------------- consulta de CEP

/**
 * Ao sair do campo de CEP com 8 dígitos, consulta o CEP e chama `preencher(dados)`.
 * `aviso` (opcional) mostra a cidade encontrada ou "consultando...".
 */
function ligarConsultaCep(campo, preencher, aviso) {
  campo.addEventListener("change", async () => {
    const cep = soDigitos(campo.value);
    if (cep.length !== 8) return;
    if (aviso) aviso.textContent = "Consultando o CEP...";
    try {
      const d = await chamar("consultar_cep", { cep });
      if (soDigitos(campo.value) !== cep) return; // o operador já trocou o CEP
      preencher(d);
      if (aviso) aviso.textContent = d.municipio_nome ? `${d.municipio_nome}/${d.uf ?? ""}` : "";
      if (!d.logradouro) toast("CEP geral da cidade: preencha logradouro e bairro.");
    } catch (e) {
      if (aviso) aviso.textContent = "";
      toast(String(e), true);
    }
  });
  campo.addEventListener("input", () => {
    if (aviso) aviso.textContent = "";
  });
}

for (const p of ["obra", "rec-obra"]) {
  ligarConsultaCep(
    $(`#${p}-cep`),
    (d) => {
      for (const [campo, valor] of [
        ["logradouro", d.logradouro],
        ["bairro", d.bairro],
        ["complemento", d.complemento],
      ]) {
        if (valor) $(`#${p}-${campo}`).value = valor;
      }
      $(d.logradouro ? `#${p}-numero` : `#${p}-logradouro`).focus();
    },
    $(`#${p}-cep-cidade`),
  );
}

ligarConsultaCep(
  $("#form-tomador").elements.cep,
  (d) => {
    const form = $("#form-tomador");
    copiarSeVeio(form, [
      ["municipio", d.municipio],
      ["logradouro", d.logradouro],
      ["complemento", d.complemento],
      ["bairro", d.bairro],
    ]);
    form.elements[d.logradouro ? "numero" : "logradouro"].focus();
  },
  $("#dlg-tomador [data-municipio-nome]"),
);

$("#form-tomador").elements.municipio.addEventListener("input", () => {
  $("#dlg-tomador [data-municipio-nome]").textContent = "";
});

ligarConsultaCnpj(
  $("#dlg-empresa"),
  "cnpj",
  () => true,
  (form, d) => {
    copiarSeVeio(form, [
      ["nome", d.razao_social],
      ["municipio", d.municipio],
      ["telefone", d.telefone],
      ["email", d.email],
    ]);
    if (d.mei) form.elements.simples.value = "mei";
    else if (d.optante_simples === true) form.elements.simples.value = "me_epp";
    else if (d.optante_simples === false) form.elements.simples.value = "nao_optante";
    if (d.municipio_nome) toast(`Município: ${d.municipio_nome}/${d.uf ?? ""}`);
  },
);

// ---------------------------------------------------------------- serviços

const NOME_RETENCAO = { irrf: "IRRF", csll: "CSLL", cp: "INSS", pis: "PIS", cofins: "COFINS" };

async function carregarServicos() {
  const corpo = $("#lista-servicos");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(6, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  estado.servicos = await invoke("listar_servicos", { empresaId: estado.empresa.id });
  renderizarServicosEmitir();
  corpo.innerHTML = estado.servicos.length
    ? estado.servicos
        .map((s, i) => {
          const ret = Object.entries(s.tributacao.retencoes ?? {})
            .map(([k, v]) => `${NOME_RETENCAO[k]} ${percentualTela(v)}%`)
            .join(", ");
          return `<tr>
            <td title="${esc(s.nome)}">${esc(s.nome)}</td>
            <td class="dado">${esc(s.codigo_trib_nac)}</td>
            <td class="dado sec-2">${esc(s.nbs ?? "")}</td>
            <td class="num">${s.valor_sugerido ? "R$ " + moeda(s.valor_sugerido) : ""}</td>
            <td class="sec-1" title="${esc(ret)}">${esc(ret || "—")}</td>
            <td>${acoesLinha(
              [
                { acao: "editar", texto: "Editar" },
                { acao: "desativar", texto: "Remover", perigo: true },
              ],
              i,
              `serviço ${s.nome}`,
            )}</td>
          </tr>`;
        })
        .join("")
    : linhaVazia(6, "Nenhum serviço cadastrado para esta empresa.", { clicar: "#novo-servico", texto: "Novo serviço" });
}

$("#lista-servicos").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const s = estado.servicos[Number(b.dataset.i)];
  if (b.dataset.acao === "editar") return editarServico(s);
  if (!(await confirmar(`Remover o serviço "${s.nome}" da lista? As notas já emitidas não mudam.`))) return;
  try {
    await chamar("desativar_servico", { id: s.id });
    await carregarServicos();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#novo-servico").addEventListener("click", () => editarServico(null));

const ctnServico = $("#servico-codigo-trib-nac");
const pontuarCtn = (c) => `${c.slice(0, 2)}.${c.slice(2, 4)}.${c.slice(4)}`;
const AJUDA_LISTA_NACIONAL = "Busca na Lista de Serviços Nacional; ao escolher, o código é preenchido.";

/** Mostra, abaixo da busca, a descrição oficial do código digitado (ou avisa que não está na lista). */
async function descreverCtn() {
  const ajuda = $("#servico-ctn-descricao");
  const codigo = soDigitos(ctnServico.value);
  if (codigo.length !== 6) {
    ajuda.textContent = AJUDA_LISTA_NACIONAL;
    return;
  }
  try {
    const { fonte, itens } = await invoke("buscar_lista_servicos", { termo: codigo });
    if (soDigitos(ctnServico.value) !== codigo) return; // o usuário já mudou o código
    ajuda.textContent = itens.length
      ? `${pontuarCtn(codigo)} · ${itens[0].descricao}`
      : `O código ${codigo} não está na Lista de Serviços Nacional embutida (${fonte}); confira antes de emitir.`;
  } catch (e) {
    toast(String(e), true);
  }
}

ctnServico.addEventListener("input", descreverCtn);

ligarCombobox($("#servico-busca-lista"), $("#servico-resultados-lista"), {
  rotulo: "Códigos de serviço",
  vazio: "Nenhum código da lista nacional encontrado.",
  contagem: (n) => `${n} código(s) encontrado(s).`,
  buscar: async (termo) => (await invoke("buscar_lista_servicos", { termo })).itens,
  opcao: (i) => `${esc(i.descricao)}<small>${pontuarCtn(i.codigo)} · ${esc(i.grupo)}</small>`,
  aoEscolher(i) {
    const busca = $("#servico-busca-lista");
    busca.value = "";
    $("#servico-resultados-lista").hidden = true;
    ctnServico.value = i.codigo;
    // o input também tira a marca de erro do campo, se havia
    ctnServico.dispatchEvent(new Event("input", { bubbles: true }));
  },
});

/** Percentual digitado ("11,48") → texto aceito pelo backend, ou null. */
function percentualDigitado(t) {
  const v = vazio(t);
  return v == null ? null : v.replace(",", ".");
}

function mostrarCamposIbs(form) {
  const informar = form.elements.ibs_informar.value === "sim";
  for (const el of $$("[data-ibs-campo]", form)) el.hidden = !informar;
}

$("#form-servico").elements.ibs_informar.addEventListener("change", () => mostrarCamposIbs($("#form-servico")));

function editarServico(s) {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  const dlg = $("#dlg-servico");
  const form = $("form", dlg);
  form.reset();
  // novo serviço: IBS/CBS ligado para o regime regular (obrigatório), desligado no Simples
  form.elements.ibs_informar.value = estado.empresa.regime.simples === "nao_optante" ? "sim" : "nao";
  if (s) {
    const t = s.tributacao;
    const ibs = t.ibs_cbs;
    const pc = t.federais?.pis_cofins ?? {};
    const r = t.retencoes ?? {};
    const tot = t.total_tributos;
    const dados = {
      nome: s.nome,
      codigo_trib_nac: s.codigo_trib_nac,
      nbs: s.nbs,
      descricao: s.descricao,
      codigo_trib_mun: s.codigo_trib_mun,
      valor_sugerido: s.valor_sugerido ? moeda(s.valor_sugerido) : "",
      trib_issqn: t.issqn?.tributacao ?? "tributavel",
      ret_issqn: t.issqn?.retencao ?? "nao_retido",
      aliq_issqn: percentualTela(t.issqn?.aliquota),
      cst: pc.cst ?? "00",
      tp_ret: String(pc.tipo_retencao ?? 0),
      r_irrf: percentualTela(r.irrf),
      r_csll: percentualTela(r.csll),
      r_cp: percentualTela(r.cp),
      r_pis: percentualTela(r.pis),
      r_cofins: percentualTela(r.cofins),
      aplicar_fed: r.aplicar ?? "sempre",
      aplicar_iss: t.aplicar_retencao_iss ?? "sempre",
      tot_tipo: "nao_informado",
      ibs_informar: ibs ? "sim" : "nao",
    };
    if (ibs) {
      Object.assign(dados, {
        ibs_ind_op: ibs.indicador_operacao,
        ibs_cst: ibs.cst,
        ibs_class: ibs.classificacao_tributaria,
        ibs_final: ibs.consumo_pessoal ? "1" : "",
      });
      // código fora da lista: acrescenta para não perder o que está gravado
      const sel = form.elements.ibs_ind_op;
      if (![...sel.options].some((o) => o.value === ibs.indicador_operacao)) {
        sel.add(new Option(ibs.indicador_operacao, ibs.indicador_operacao));
      }
    }
    if (tot && typeof tot === "object" && "percentual_simples" in tot) {
      dados.tot_tipo = "percentual_simples";
      dados.tot_sn = percentualTela(tot.percentual_simples);
    } else if (tot && typeof tot === "object" && "percentuais" in tot) {
      dados.tot_tipo = "percentuais";
      dados.tot_fed = percentualTela(tot.percentuais.federal);
      dados.tot_est = percentualTela(tot.percentuais.estadual);
      dados.tot_mun = percentualTela(tot.percentuais.municipal);
    }
    preencherForm(form, dados);
  }
  mostrarCamposIbs(form);
  descreverCtn();
  abrirDialogo(dlg, $("#erro-servico"), async (d) => {
    const retencoes = { aplicar: d.aplicar_fed };
    for (const k of ["irrf", "csll", "cp", "pis", "cofins"]) {
      const v = percentualDigitado(d["r_" + k]);
      if (v != null) retencoes[k] = v;
    }
    let total_tributos = "nao_informado";
    if (d.tot_tipo === "percentual_simples") {
      const p = percentualDigitado(d.tot_sn);
      if (p == null) throw new ErroCampo("tot_sn", 'Informe o percentual do Simples (ou escolha "Não informar").');
      total_tributos = { percentual_simples: p };
    } else if (d.tot_tipo === "percentuais") {
      total_tributos = {
        percentuais: {
          federal: percentualDigitado(d.tot_fed) ?? "0",
          estadual: percentualDigitado(d.tot_est) ?? "0",
          municipal: percentualDigitado(d.tot_mun) ?? "0",
        },
      };
    }
    const issqn = { tributacao: d.trib_issqn, retencao: d.ret_issqn };
    const aliq = percentualDigitado(d.aliq_issqn);
    if (aliq != null) issqn.aliquota = aliq;
    const ibs_cbs =
      d.ibs_informar === "sim"
        ? {
            ...(s?.tributacao?.ibs_cbs ?? {}), // preserva o indicador de destinatário
            indicador_operacao: d.ibs_ind_op,
            cst: soDigitos(d.ibs_cst),
            classificacao_tributaria: soDigitos(d.ibs_class),
            consumo_pessoal: d.ibs_final === "1" ? true : null,
          }
        : null;
    const servico = {
      id: s?.id ?? null,
      empresa_id: estado.empresa.id,
      nome: d.nome.trim(),
      descricao: d.descricao.trim(),
      codigo_trib_nac: d.codigo_trib_nac,
      codigo_trib_mun: vazio(d.codigo_trib_mun),
      nbs: vazio(d.nbs),
      tributacao: {
        ...(s?.tributacao ?? {}), // preserva grupos sem campo na tela
        issqn,
        federais: {
          ...(s?.tributacao?.federais ?? {}),
          pis_cofins: { cst: d.cst.trim() || "00", tipo_retencao: Number(d.tp_ret) },
        },
        total_tributos,
        retencoes,
        aplicar_retencao_iss: d.aplicar_iss,
        ibs_cbs,
      },
      valor_sugerido: valorDigitado(d.valor_sugerido),
    };
    await chamar("salvar_servico", { servico });
    toast("Serviço salvo.");
    return carregarServicos;
  });
}

// ---------------------------------------------------------------- importar / exportar serviços

/** Serviços mostrados no diálogo de importação: [{ servico, erros, linha? }]. */
let servicosAImportar = [];

$("#exportar-servicos").addEventListener("click", async () => {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  try {
    toast(`Planilha salva em ${await chamar("exportar_servicos", { empresaId: estado.empresa.id })}`);
  } catch (e) {
    toast(String(e), true);
  }
});

$("#modelo-servicos").addEventListener("click", async () => {
  try {
    toast(`Modelo salvo em ${await chamar("salvar_modelo_servicos")}`);
  } catch (e) {
    toast(String(e), true);
  }
});

function desenharServicosAImportar(vazioTexto) {
  const corpo = $("#lista-importar-servicos");
  $("#importar-todos").checked = false;
  corpo.innerHTML = servicosAImportar.length
    ? servicosAImportar
        .map((it, i) => {
          const s = it.servico;
          const ok = !it.erros.length;
          const situacao = ok
            ? `<span class="selo autorizada">Pronto</span>`
            : `<span class="selo rejeitada" title="${esc(it.erros.join("\n"))}">${esc(it.erros.join("; "))}</span>`;
          const alvo = it.linha ? `linha ${it.linha}: ${s.nome}` : s.nome;
          return `<tr>
            <td><input type="checkbox" data-i="${i}" ${ok ? "checked" : "disabled"} aria-label="${esc(`Marcar ${alvo}`)}"></td>
            <td title="${esc(alvo)}">${esc(s.nome || "(sem nome)")}</td>
            <td class="dado">${esc(s.codigo_trib_nac)}</td>
            <td class="num">${s.valor_sugerido ? "R$ " + moeda(s.valor_sugerido) : ""}</td>
            <td class="sec-1">${situacao}</td>
          </tr>`;
        })
        .join("")
    : `<tr><td colspan="5" class="vazio">${esc(vazioTexto)}</td></tr>`;
  selecaoImportar.atualizar();
}

const selecaoImportar = selecaoEmTabela({
  lista: "#lista-importar-servicos",
  itens: () => servicosAImportar,
  botao: "#importar-servicos-ok",
  rotulo: "Importar",
  todos: "#importar-todos",
});

async function carregarOrigemImportacao() {
  const form = $("#form-importar-servicos");
  const origem = form.elements.origem.value;
  for (const el of $$("[data-origem]", form)) el.hidden = el.dataset.origem !== origem;
  const origemId = origem === "empresa" ? Number(form.elements.empresa_origem.value) : null;
  // A empresa de origem não aparece como destino.
  for (const c of $$("#importar-destinos input")) {
    const mesma = Number(c.value) === origemId;
    c.closest("label").hidden = mesma;
    if (mesma) c.checked = false;
  }
  servicosAImportar = [];
  if (origem === "empresa") {
    if (!origemId) return desenharServicosAImportar("Cadastre outra empresa para copiar os serviços dela.");
    try {
      const lista = await invoke("listar_servicos", { empresaId: origemId });
      servicosAImportar = lista.map((servico) => ({ servico, erros: [] }));
    } catch (e) {
      return desenharServicosAImportar(String(e));
    }
    return desenharServicosAImportar("Esta empresa não tem serviços cadastrados.");
  }
  const arquivo = $("#importar-arquivo").files[0];
  if (!arquivo) return desenharServicosAImportar("Escolha a planilha (.xlsx, .xls, .ods ou .csv).");
  try {
    const bytes = Array.from(new Uint8Array(await arquivo.arrayBuffer()));
    servicosAImportar = await chamar("ler_planilha_servicos", { nome: arquivo.name, bytes });
  } catch (e) {
    return desenharServicosAImportar(String(e));
  }
  desenharServicosAImportar("Nenhuma linha com dados na planilha.");
}

$("#importar-origem").addEventListener("change", carregarOrigemImportacao);
$("#importar-empresa").addEventListener("change", carregarOrigemImportacao);
$("#importar-arquivo").addEventListener("change", carregarOrigemImportacao);

$("#importar-servicos").addEventListener("click", () => {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  const dlg = $("#dlg-importar-servicos");
  const form = $("form", dlg);
  form.reset();
  const outras = estado.empresas.filter((e) => e.id !== estado.empresa.id);
  form.elements.empresa_origem.innerHTML = outras.length
    ? outras.map((e) => `<option value="${e.id}">${esc(e.nome)} — ${esc(e.cnpj_formatado)}</option>`).join("")
    : `<option value="">Nenhuma outra empresa cadastrada</option>`;
  form.elements.origem.value = outras.length ? "empresa" : "planilha";
  $("#importar-destinos").innerHTML = estado.empresas
    .map(
      (e) => `<label class="linha-check c6"><input type="checkbox" value="${e.id}" ${e.id === estado.empresa.id ? "checked" : ""}>
        ${esc(e.nome)} — ${esc(e.cnpj_formatado)}</label>`,
    )
    .join("");
  carregarOrigemImportacao();

  abrirDialogo(dlg, $("#erro-importar-servicos"), async () => {
    const servicos = selecaoImportar.marcados().map((it) => it.servico);
    if (!servicos.length) throw "Marque os serviços a importar.";
    const empresaIds = $$("#importar-destinos input:checked").map((c) => Number(c.value));
    if (!empresaIds.length) throw "Marque ao menos uma empresa em \"Importar para\".";
    const resultado = await chamar("importar_servicos", { servicos, empresaIds });
    const linhas = resultado.map((r) => `${r.empresa}: ${r.novos} novo(s), ${r.atualizados} atualizado(s)`);
    toast(linhas.join(" · "));
    const avisos = resultado.flatMap((r) => r.avisos.map((a) => `${r.empresa} — ${a}`));
    if (avisos.length) {
      alert(`Importado, mas estes serviços precisam de ajuste antes de emitir:\n\n${avisos.join("\n")}`);
    }
    if (empresaIds.includes(estado.empresa.id)) return carregarServicos;
  });
});

// ---------------------------------------------------------------- empresas

async function renderizarEmpresas() {
  const corpo = $("#lista-empresas");
  corpo.innerHTML = estado.empresas.length
    ? estado.empresas
        .map((e, i) => {
          const c = e.certificado;
          // validade em uma linha; titular no title; carimbo só quando vence em até 30 dias
          let cert = `<span class="selo rejeitada">Sem certificado</span>`;
          if (c) {
            const d = c.dias_para_vencer;
            const carimbo =
              d < 0
                ? `<span class="selo rejeitada">Vencido</span>`
                : d <= 30
                  ? `<span class="selo incerta">${d} ${d === 1 ? "dia" : "dias"}</span>`
                  : "";
            cert = `<span class="dado">${esc(c.valido_ate)}</span> ${carimbo}`;
          }
          return `<tr>
            <td class="dado sec-2">${esc(e.cnpj_formatado)}</td>
            <td title="${esc(`${e.nome} — ${e.cnpj_formatado}`)}">${esc(e.nome)}</td>
            <td><span class="selo ${e.ambiente}">${NOME_AMBIENTE[e.ambiente]}</span></td>
            <td class="dado sec-1">${esc(e.serie)}</td>
            <td title="${esc(c ? `Titular: ${c.titular}` : "Importe o certificado A1 da empresa")}">${cert}</td>
            <td>${acoesLinha(
              [
                { acao: "editar", texto: "Editar" },
                { acao: "certificado", texto: "Certificado" },
                { acao: "numeracao", texto: "Numeração / IM" },
                { acao: "excluir", texto: "Excluir", perigo: true },
              ],
              i,
              e.nome,
            )}</td>
          </tr>`;
        })
        .join("")
    : linhaVazia(6, "Nenhuma empresa cadastrada.", { clicar: "#nova-empresa", texto: "Nova empresa" });
}

$("#lista-empresas").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const e = estado.empresas[Number(b.dataset.i)];
  if (b.dataset.acao === "certificado") importarCertificado(e);
  else if (b.dataset.acao === "excluir") excluirEmpresa(e);
  else if (b.dataset.acao === "editar") editarEmpresa(e);
  else editarNumeracao(e);
});

async function excluirEmpresa(empresa) {
  const pergunta =
    `Excluir ${empresa.nome} (${empresa.cnpj_formatado})?\n\n` +
    "Sem notas emitidas, o cadastro, o certificado e os serviços são apagados.\n" +
    "Com notas emitidas, a empresa só é arquivada e as notas continuam guardadas.";
  if (!(await confirmar(pergunta))) return;
  try {
    toast(await chamar("excluir_empresa", { empresaId: empresa.id }));
    if (estado.empresa?.id === empresa.id) estado.empresa = null;
    await carregarEmpresas();
    await renderizarEmpresas();
  } catch (e) {
    toast(String(e), true);
  }
}

$("#nova-empresa").addEventListener("click", () => editarEmpresa(null));

/** Cadastro (`empresa` nulo) ou edição de empresa no mesmo diálogo. */
function editarEmpresa(empresa) {
  const dlg = $("#dlg-empresa");
  const form = $("form", dlg);
  form.reset();
  $("[data-aviso-cnpj]", dlg).hidden = true;
  $("#titulo-empresa").textContent = empresa ? "Editar empresa" : "Nova empresa";
  form.elements.cnpj.readOnly = !!empresa;
  $("[data-consultar-cnpj]", dlg).textContent = "Consultar";
  if (empresa) {
    preencherForm(form, {
      cnpj: empresa.cnpj_formatado,
      nome: empresa.nome,
      im: empresa.inscricao_municipal,
      municipio: empresa.municipio_emissor,
      serie: empresa.serie,
      simples: empresa.regime.simples,
      apuracao: empresa.regime.apuracao_simples ?? "",
      regime_especial: String(empresa.regime.regime_especial ?? 0),
      ambiente: empresa.ambiente,
      telefone: empresa.telefone,
      email: empresa.email,
    });
  }
  abrirDialogo(dlg, $("#erro-empresa"), async (d) => {
    if (d.simples === "me_epp" && !d.apuracao) throw new ErroCampo("apuracao", "Para ME/EPP optante, escolha a forma de apuração.");
    const regime = { simples: d.simples, regime_especial: Number(d.regime_especial) };
    if (d.simples === "me_epp") regime.apuracao_simples = d.apuracao;
    const dados = {
      cnpj: empresa ? empresa.cnpj : d.cnpj.replace(/[^0-9A-Za-z]/g, "").toUpperCase(),
      nome: d.nome.trim(),
      inscricao_municipal: vazio(d.im),
      municipio_emissor: soDigitos(d.municipio),
      regime,
      ambiente: d.ambiente,
      serie: d.serie,
      telefone: vazio(soDigitos(d.telefone)),
      email: vazio(d.email),
    };
    const vaiParaProducao = d.ambiente === "producao" && empresa?.ambiente !== "producao";
    const mudaEmissao = empresa && (d.ambiente !== empresa.ambiente || d.serie.trim() !== empresa.serie);
    if (vaiParaProducao && !(await confirmar(`${empresa ? "Passar" : "Cadastrar"} ${d.nome.trim()} em PRODUÇÃO? As notas emitidas terão valor fiscal.`))) {
      throw "Não confirmado.";
    }
    if (!empresa) {
      const id = await chamar("cadastrar_empresa", { empresa: dados });
      toast("Empresa cadastrada. Agora importe o certificado digital.");
      estado.empresa = { id };
      await carregarEmpresas();
      await renderizarEmpresas();
      const nova = estado.empresas.find((x) => x.id === id);
      if (nova) setTimeout(() => importarCertificado(nova), 0);
      return;
    }
    await chamar("atualizar_empresa", { empresaId: empresa.id, empresa: dados });
    toast(
      mudaEmissao
        ? `Empresa agora emite em ${NOME_AMBIENTE[d.ambiente]}, série ${d.serie.trim()}. Confira o próximo número e a IM em "Numeração / IM".`
        : "Empresa atualizada.",
    );
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}

function importarCertificado(empresa) {
  const dlg = $("#dlg-certificado");
  $("form", dlg).reset();
  $("#cert-empresa").textContent = `${empresa.nome} — ${empresa.cnpj_formatado}`;
  abrirDialogo(dlg, $("#erro-certificado"), async (d, form) => {
    const arquivo = form.elements.arquivo.files[0];
    if (!arquivo) throw "Escolha o arquivo do certificado.";
    const pfx = Array.from(new Uint8Array(await arquivo.arrayBuffer()));
    const info = await chamar("importar_certificado", { empresaId: empresa.id, pfx, senha: d.senha });
    form.elements.senha.value = "";
    toast(`Certificado de ${info.titular} importado (válido até ${info.valido_ate}).`);
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}

function editarNumeracao(empresa) {
  const dlg = $("#dlg-numeracao");
  const form = $("form", dlg);
  form.reset();
  $("#num-empresa").textContent =
    `${empresa.nome} — série ${empresa.serie} em ${NOME_AMBIENTE[empresa.ambiente]}` +
    (empresa.inscricao_municipal ? ` — IM ${empresa.inscricao_municipal}` : " — sem inscrição municipal cadastrada");
  preencherForm(form, {
    proximo: "",
    im_prod: empresa.im_na_dps_producao,
    im_hom: empresa.im_na_dps_homologacao,
  });
  form.elements.proximo.placeholder = "deixe em branco para não mudar";
  const botaoSefin = $("[data-ajustar-sefin]", dlg);
  botaoSefin.onclick = async () => {
    botaoSefin.disabled = true;
    botaoSefin.textContent = "Consultando...";
    try {
      const msg = await chamar("ajustar_numeracao", { empresaId: empresa.id });
      toast(msg);
      $("#num-empresa").textContent = msg;
    } catch (e) {
      mostrarErro($("#erro-numeracao"), e);
    } finally {
      botaoSefin.disabled = false;
      botaoSefin.textContent = "Consultar na Sefin";
    }
  };
  abrirDialogo(dlg, $("#erro-numeracao"), async (d) => {
    if (vazio(d.proximo)) {
      const proximo = Number(d.proximo);
      if (!Number.isInteger(proximo) || proximo < 1) throw new ErroCampo("proximo", "Número inteiro, maior que zero.");
      if (!(await confirmar(`Próxima DPS da série ${empresa.serie} será a nº ${proximo}. Confirma?`))) throw "Não confirmado.";
      await chamar("definir_proximo_numero", {
        empresaId: empresa.id,
        ambiente: empresa.ambiente,
        serie: empresa.serie,
        proximo,
      });
    }
    if (d.im_prod !== empresa.im_na_dps_producao) {
      await chamar("definir_im_na_dps", { empresaId: empresa.id, ambiente: "producao", informar: d.im_prod });
    }
    if (d.im_hom !== empresa.im_na_dps_homologacao) {
      await chamar("definir_im_na_dps", { empresaId: empresa.id, ambiente: "homologacao", informar: d.im_hom });
    }
    toast("Configuração salva.");
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}

// ---------------------------------------------------------------- configurações

async function carregarConfiguracoes() {
  $("#ap-zoom").value = aparencia.zoom;
  $("#ap-densidade").value = aparencia.densidade;
  $("#pasta-saida").value = await invoke("obter_pasta");
  await carregarBackup();
  await carregarEmail();
  const venc = await invoke("certificados_vencendo", { dias: 60 });
  $("#lista-vencimentos").innerHTML = venc.length
    ? venc
        .map(
          (v) => `<tr><td>${esc(v.empresa)}</td><td>${esc(v.valido_ate)}</td>
            <td class="num alerta">${v.dias < 0 ? "vencido" : v.dias + " dia(s)"}</td></tr>`,
        )
        .join("")
    : `<tr><td class="vazio">Nenhum certificado vence nos próximos 60 dias.</td></tr>`;
  await carregarPendentes();
}

async function carregarPendentes() {
  const lista = await invoke("pendentes");
  const nomeEmpresa = (id) => estado.empresas.find((e) => e.id === id)?.nome ?? `empresa ${id}`;
  $("#lista-pendentes").innerHTML = lista.length
    ? lista
        .map(
          (p) => `<tr><td>${esc(nomeEmpresa(p.empresa_id))}</td><td><code>${esc(p.id_dps)}</code>
            <div class="ajuda">${esc(p.erro ?? "")}</div></td>
            <td><button class="botao pequeno" data-dps="${p.dps_id}">Verificar</button></td></tr>`,
        )
        .join("")
    : `<tr><td class="vazio">Nenhum envio pendente.</td></tr>`;
}

$("#lista-pendentes").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-dps]");
  if (!b) return;
  b.disabled = true;
  try {
    toast(await chamar("reconciliar", { dpsId: Number(b.dataset.dps) }));
    await carregarPendentes();
  } catch (e) {
    toast(String(e), true);
    b.disabled = false;
  }
});

async function carregarEmail() {
  const { config, tem_senha } = await invoke("obter_email");
  const form = $("#form-email");
  preencherForm(form, { ...config, senha: "", usuario: config.usuario ?? "", nome_remetente: config.nome_remetente ?? "" });
  $("[data-senha-guardada]", form).hidden = !tem_senha;
}

$("#form-email").addEventListener("submit", (ev) => ev.preventDefault());

$("#form-email").elements.seguranca.addEventListener("change", (ev) => {
  const porta = $("#form-email").elements.porta;
  porta.value = { start_tls: 587, ssl: 465, nenhuma: 25 }[ev.target.value] ?? porta.value;
});

function lerConfigEmail() {
  const d = lerForm($("#form-email"));
  return {
    config: {
      servidor: d.servidor.trim(),
      porta: Number(d.porta),
      seguranca: d.seguranca,
      usuario: vazio(d.usuario),
      remetente: d.remetente.trim(),
      nome_remetente: vazio(d.nome_remetente),
      assunto: d.assunto,
      corpo: d.corpo,
      enviar_ao_emitir: d.enviar_ao_emitir,
    },
    senha: d.senha || null,
  };
}

/** Confere os campos do e-mail (e, no teste, o destinatário); devolve false e marca o erro se algo está errado. */
function emailConfigOk(...extras) {
  const form = $("#form-email");
  limparErros(form);
  for (const el of extras) limparErro(el);
  const erros = [...validarCampos(form), ...extras.flatMap((el) => validarCampos(el.parentElement).filter((e) => e.el === el))];
  if (erros.length) mostrarErros(erros);
  return !erros.length;
}

$("#salvar-email").addEventListener("click", async () => {
  if (!emailConfigOk()) return;
  try {
    await chamar("salvar_email", lerConfigEmail());
    toast("Configuração de e-mail salva.");
    await carregarEmail();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#testar-email").addEventListener("click", async () => {
  if (!emailConfigOk($("#email-teste"))) return;
  const para = $("#email-teste").value.trim();
  const botao = $("#testar-email");
  botao.disabled = true;
  botao.textContent = "Enviando...";
  try {
    // Salva antes, para testar exatamente o que está na tela.
    await chamar("salvar_email", lerConfigEmail());
    await chamar("testar_email", { para });
    toast(`Teste enviado para ${para}. Confira a caixa de entrada (e o spam).`);
    await carregarEmail();
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
    botao.textContent = "Enviar teste";
  }
});

// "Escolher pasta…": o seletor só preenche o campo (cancelar não muda nada); salvar valida como antes.
document.querySelectorAll("[data-escolher]").forEach((botao) => {
  botao.addEventListener("click", async () => {
    const campo = $(botao.dataset.escolher);
    try {
      const pasta = await invoke("escolher_pasta", { inicial: campo.value });
      if (pasta) campo.value = pasta;
    } catch (e) {
      toast(String(e), true);
    }
  });
});

$("#salvar-pasta").addEventListener("click", async () => {
  try {
    await chamar("definir_pasta", { pasta: $("#pasta-saida").value });
    toast("Pasta salva.");
  } catch (e) {
    toast(String(e), true);
  }
});

async function carregarBackup() {
  const b = await invoke("obter_backup");
  $("#pasta-backup").value = b.pasta;
  $("#ultimo-backup").textContent = b.ultimo
    ? `Último backup: ${new Date(b.ultimo).toLocaleString("pt-BR")}.`
    : b.pasta
      ? "Nenhum backup feito ainda."
      : "Defina a pasta para ativar o backup automático.";
}

$("#salvar-pasta-backup").addEventListener("click", async () => {
  try {
    await chamar("definir_pasta_backup", { pasta: $("#pasta-backup").value });
    toast("Pasta do backup salva.");
    await carregarBackup();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#fazer-backup").addEventListener("click", async () => {
  const botao = $("#fazer-backup");
  botao.disabled = true;
  try {
    const nome = await chamar("fazer_backup");
    toast(`Backup gravado: ${nome}`);
    await carregarBackup();
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
  }
});

$("#restaurar-backup").addEventListener("click", async () => {
  const botao = $("#restaurar-backup");
  botao.disabled = true;
  try {
    await chamar("restaurar_backup", {
      arquivo: $("#restaurar-arquivo").value,
      senha: $("#restaurar-senha").value,
      destino: $("#restaurar-destino").value,
    });
    $("#restaurar-senha").value = "";
    toast("Backup aberto e íntegro. O banco foi gerado no destino informado.");
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
  }
});

$("#abrir-pasta").addEventListener("click", async () => {
  try {
    await invoke("abrir_arquivo", { caminho: $("#pasta-saida").value });
  } catch (e) {
    toast(String(e), true);
  }
});

// ---------------------------------------------------------------- atalhos e preferências do dia a dia

const TELAS = $$("nav button[data-tela]").map((b) => b.dataset.tela);
const telaAtual = () => $$("section[data-tela]").find((s) => !s.hidden)?.dataset.tela;

// o atalho aparece no title dos botões e é informado ao leitor de tela
TELAS.forEach((tela, i) => {
  const b = $(`nav button[data-tela="${tela}"]`);
  b.title = `${b.title} (Alt+${i + 1})`;
  b.setAttribute("aria-keyshortcuts", `Alt+${i + 1}`);
});
for (const [sel, texto] of [
  ["#revisar", "Revisar (Ctrl+Enter)"],
  ["#confirmar-emissao", "Emitir nota (Ctrl+Enter)"],
]) {
  $(sel).title = texto;
  $(sel).setAttribute("aria-keyshortcuts", "Control+Enter");
}

/** Ctrl+N: tela de emissão limpa, com o foco na busca do tomador (mantém o serviço escolhido). */
function novaNota() {
  irPara("emitir");
  limparEmissao();
  $("#termo-tomador").focus();
}

document.addEventListener("keydown", (ev) => {
  if ($("#app").hidden || ev.defaultPrevented) return;
  const emDialogo = !!document.querySelector("dialog[open]");
  // F5 e Ctrl+R recarregariam a página (e trancariam o sistema): recarregam só a lista da tela
  if (ev.key === "F5" || (ev.ctrlKey && !ev.altKey && ev.key.toLowerCase() === "r")) {
    ev.preventDefault();
    if (!emDialogo && telaAtual()) irPara(telaAtual());
    return;
  }
  if (emDialogo) return;
  if (ev.altKey && !ev.ctrlKey && /^[1-9]$/.test(ev.key)) {
    const tela = TELAS[Number(ev.key) - 1];
    if (tela) {
      ev.preventDefault();
      irPara(tela, true);
    }
    return;
  }
  if (!ev.ctrlKey || ev.altKey || ev.shiftKey) return;
  const tecla = ev.key.toLowerCase();
  if (tecla === "n") {
    ev.preventDefault();
    novaNota();
  } else if (tecla === "f") {
    const busca = $({ emitir: "#termo-tomador", tomadores: "#busca-tomadores" }[telaAtual()] ?? "#nenhum");
    if (busca?.getClientRects().length) {
      ev.preventDefault();
      busca.focus();
      busca.select();
    }
  } else if (ev.key === "Enter" && telaAtual() === "emitir") {
    // primeiro Enter revisa; com a revisão aberta, o segundo emite
    ev.preventDefault();
    const confirmar = $("#confirmar-emissao");
    if (!$("#revisao").hidden && !confirmar.disabled) confirmar.click();
    else $("#revisar").click();
  }
});

/** Mês escolhido numa tela, lembrado só até o fim do mês corrente (no mês seguinte volta ao padrão). */
function mesLembrado(chave, padrao) {
  try {
    const v = JSON.parse(lerPreferencia(chave) ?? "null");
    if (v && v.em === mesAtual() && typeof v.mes === "string") return v.mes;
  } catch {
    /* preferência ilegível: usa o padrão */
  }
  return padrao;
}
for (const [campo, chave] of [
  ["#mes-notas", "mes-notas"],
  ["#mes-lote", "mes-lote"],
]) {
  $(campo).addEventListener("change", (ev) =>
    gravarPreferencia(chave, JSON.stringify({ mes: ev.target.value, em: mesAtual() })),
  );
}
$("#todas-notas").addEventListener("click", () =>
  gravarPreferencia("mes-notas", JSON.stringify({ mes: "", em: mesAtual() })),
);

/** Linha de "nenhum registro" com a ação que resolve: clica num botão da tela ou vai para outra tela. */
function linhaVazia(colunas, texto, acao) {
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

// ---------------------------------------------------------------- aparência (tamanho do texto e densidade)

const ZOOMS = ["0.9", "1", "1.1", "1.25", "1.4", "1.5"];
let aparencia = { zoom: "1", densidade: "auto" };

function aplicarAparencia() {
  document.documentElement.style.setProperty("--zoom", aparencia.zoom);
  document.documentElement.dataset.densidade = aparencia.densidade;
  atualizarMenu(); // o limite do menu é em rem: muda com o tamanho do texto
}

async function salvarAparencia() {
  aplicarAparencia();
  try {
    await invoke("salvar_aparencia", { aparencia });
  } catch (e) {
    toast(String(e), true);
  }
}

/** Ctrl + / Ctrl − / Ctrl 0 e Ctrl + roda do mouse; `passo` 0 volta a 100%. */
function mudarZoom(passo) {
  const i = ZOOMS.indexOf(aparencia.zoom);
  const novo = passo === 0 ? "1" : ZOOMS[Math.min(ZOOMS.length - 1, Math.max(0, (i < 0 ? 1 : i) + passo))];
  if (novo === aparencia.zoom) return;
  aparencia.zoom = novo;
  $("#ap-zoom").value = novo;
  salvarAparencia();
  toast(`Tamanho do texto: ${Math.round(Number(novo) * 100)}%`);
}

document.addEventListener("keydown", (ev) => {
  if (!ev.ctrlKey || ev.altKey || ev.metaKey) return;
  const passo = { "+": 1, "=": 1, "-": -1, "0": 0 }[ev.key];
  if (passo === undefined) return;
  ev.preventDefault();
  mudarZoom(passo);
});

document.addEventListener(
  "wheel",
  (ev) => {
    if (!ev.ctrlKey) return;
    ev.preventDefault();
    mudarZoom(ev.deltaY < 0 ? 1 : -1);
  },
  { passive: false },
);

$("#ap-zoom").addEventListener("change", (ev) => {
  aparencia.zoom = ev.target.value;
  salvarAparencia();
});

$("#ap-densidade").addEventListener("change", (ev) => {
  aparencia.densidade = ev.target.value;
  salvarAparencia();
});

// ---------------------------------------------------------------- atualização do sistema

let atualizacao = null; // { versao, notas } da Release mais nova, se houver

function mostrarSituacaoAtualizacao(texto) {
  $("#situacao-atualizacao").textContent = texto;
  $("#instalar-atualizacao").hidden = !atualizacao;
}

/** Consulta a Release mais nova. `silencioso`: na abertura, sem internet não é erro. */
async function procurarAtualizacao(silencioso) {
  try {
    atualizacao = await invoke("procurar_atualizacao");
  } catch (e) {
    if (!silencioso) mostrarSituacaoAtualizacao(`Não foi possível procurar: ${e}`);
    return;
  }
  mostrarSituacaoAtualizacao(atualizacao ? `Versão ${atualizacao.versao} disponível.` : "O sistema está na versão mais nova.");
  if (atualizacao && !$("#aviso-atualizacao")) {
    $("#alertas").insertAdjacentHTML(
      "beforeend",
      `<div class="aviso ok" id="aviso-atualizacao"><span><strong>Versão ${esc(atualizacao.versao)} disponível.</strong>
        O sistema fecha, instala e abre de novo.</span>
        <button type="button" class="botao pequeno" data-instalar-atualizacao>Atualizar agora</button></div>`,
    );
  }
}

/** Algo foi (ou está indo) para a Sefin: fechar o sistema agora deixaria a nota sem registro da resposta. */
function envioEmCurso() {
  return loteEmAndamento || planilhaEmAndamento || emissaoEmCurso || cancelamentoEmCurso;
}

/** Durante o download e a instalação nenhuma emissão começa (o sistema fecha no fim). */
let atualizando = false;

async function instalarAtualizacao() {
  if (!atualizacao || atualizando) return;
  const espere = () => toast("Espere terminar a emissão em andamento antes de atualizar.", true);
  if (envioEmCurso()) return espere();
  if (!(await confirmar(`Atualizar para a versão ${atualizacao.versao}? O sistema fecha, instala e abre de novo.`))) return;
  // a emissão pode ter começado enquanto a confirmação estava aberta
  if (envioEmCurso()) return espere();
  atualizando = true;
  const botoes = $$("#instalar-atualizacao, [data-instalar-atualizacao]");
  for (const b of botoes) b.disabled = true;
  mostrarSituacaoAtualizacao("Baixando a atualização...");
  const parar = await window.__TAURI__.event.listen("progresso-atualizacao", ({ payload }) => {
    const mb = (n) => (n / 1048576).toFixed(1).replace(".", ",");
    mostrarSituacaoAtualizacao(
      payload.total
        ? `Baixando a atualização... ${Math.floor((payload.baixado * 100) / payload.total)}% (${mb(payload.total)} MB)`
        : `Baixando a atualização... ${mb(payload.baixado)} MB`,
    );
  });
  try {
    await invoke("instalar_atualizacao");
    mostrarSituacaoAtualizacao("Instalando...");
  } catch (e) {
    mostrarSituacaoAtualizacao(String(e));
    toast(String(e), true);
    for (const b of botoes) b.disabled = false;
    atualizando = false;
  } finally {
    parar();
  }
}

$("#procurar-atualizacao").addEventListener("click", async (ev) => {
  ev.target.disabled = true;
  mostrarSituacaoAtualizacao("Procurando...");
  await procurarAtualizacao(false);
  ev.target.disabled = false;
});
$("#instalar-atualizacao").addEventListener("click", instalarAtualizacao);
$("#alertas").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-instalar-atualizacao]")) instalarAtualizacao();
});

// ---------------------------------------------------------------- início

invoke("versao_app")
  .then((v) => ($("#versao-app").textContent = v))
  .catch(() => {});

invoke("obter_aparencia")
  .then((a) => {
    aparencia = a;
    aplicarAparencia();
  })
  .catch(() => {});
iniciarTrava().catch((e) => mostrarErro($("#erro-trava"), e));
