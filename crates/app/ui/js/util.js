// Utilitários comuns: comandos Tauri, seletores, estado da sessão, valores e datas.

export const invoke = window.__TAURI__.core.invoke;

export const $ = (sel, raiz = document) => raiz.querySelector(sel);
export const $$ = (sel, raiz = document) => [...raiz.querySelectorAll(sel)];

export const estado = {
  empresas: [],
  empresa: null,
  servicos: [],
  tomador: null, // TomadorDto escolhido na tela de emitir
  pedido: null, // último pedido revisado
  reter: null, // { federais, iss } escolhido na revisão; null = cadastro do tomador
  mesPadrao: "atual", // mês de referência sugerido nas telas: "atual" ou "anterior" (Configurações)
};

// ---------------------------------------------------------------- utilitários

export function esc(texto) {
  return String(texto ?? "").replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
  );
}

/** Confirmação some em 4 s; erro fica até o operador fechar (e é anunciado como alerta). */
export function toast(texto, erro = false) {
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

// ---------------------------------------------------------------- utilitários gerais: comandos, valores, datas

/** Chama um comando mostrando o cursor de espera; o erro volta como texto. */
export async function chamar(comando, args) {
  document.body.classList.add("ocupado");
  $("main")?.setAttribute("aria-busy", "true");
  try {
    return await invoke(comando, args);
  } finally {
    document.body.classList.remove("ocupado");
    $("main")?.removeAttribute("aria-busy");
  }
}

/** Copia para a área de transferência; sem a API moderna, usa um campo temporário. */
export async function copiarTexto(texto) {
  try {
    await navigator.clipboard.writeText(texto);
    return;
  } catch {
    const campo = document.createElement("textarea");
    campo.value = texto;
    campo.setAttribute("readonly", "");
    campo.className = "sr";
    document.body.append(campo);
    campo.select();
    const ok = document.execCommand("copy");
    campo.remove();
    if (!ok) throw new Error("não foi possível copiar; selecione o texto e copie à mão");
  }
}

export function mostrarErro(el, erro) {
  el.textContent = String(erro);
  el.hidden = false;
}

/** "150.00" → "150,00" com separador de milhar. */
export function moeda(valor) {
  if (valor == null || valor === "") return "";
  const [inteiro, frac = "00"] = String(valor).split(".");
  return inteiro.replace(/\B(?=(\d{3})+(?!\d))/g, ".") + "," + frac.padEnd(2, "0");
}

/**
 * Soma valores do backend ("1250.00") em centavos, sem ponto flutuante; devolve no mesmo formato.
 * `campo(item)` lê o valor de cada item (nulo conta zero).
 */
export function somarValores(itens, campo) {
  let centavos = 0n;
  for (const item of itens) {
    const [inteiro, frac = ""] = String(campo(item) ?? "0").split(".");
    centavos += BigInt(inteiro || "0") * 100n + BigInt((frac + "00").slice(0, 2));
  }
  return `${centavos / 100n}.${String(centavos % 100n).padStart(2, "0")}`;
}

/** "1.500,00" → "1500,00" (o backend aceita vírgula decimal). Vazio → null. */
export function valorDigitado(texto) {
  const t = String(texto ?? "").trim().replace(/\s|R\$/g, "");
  if (!t) return null;
  return t.includes(",") ? t.replace(/\./g, "") : t;
}

/** "11.48" → "11,48" para campos de percentual. */
export function percentualTela(p) {
  return p == null ? "" : String(p).replace(".", ",");
}

export const soDigitos = (t) => String(t ?? "").replace(/\D/g, "");
export const vazio = (t) => (String(t ?? "").trim() === "" ? null : String(t).trim());

export function formatarDocumento(doc) {
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

export function mesAnterior() {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export function mesAtual() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** Mês de referência que as telas sugerem, conforme a configuração (`AAAA-MM`). */
export const mesPadrao = () => (estado.mesPadrao === "anterior" ? mesAnterior() : mesAtual());

export const NOME_AMBIENTE = { producao: "PRODUÇÃO", homologacao: "Homologação" };
export const NOME_SITUACAO = {
  autorizada: "Autorizada",
  rejeitada: "Rejeitada",
  incerta: "Sem resposta",
  enviando: "Enviando",
  cancelada: "Cancelada",
  substituida: "Substituída",
};
