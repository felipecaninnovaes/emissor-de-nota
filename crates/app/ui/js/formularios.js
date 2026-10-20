// Formulários: formatos, máscaras, validação com erro junto do campo e diálogos.

import { $, $$, estado, moeda, mostrarErro, soDigitos, toast, valorDigitado } from "./util.js";

/** Erro de um campo específico: aparece embaixo dele em vez de no aviso geral do diálogo. */
export class ErroCampo extends Error {
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
/** Código de tabela oficial em campo com busca: texto com letras é busca pelo nome e fica como está. */
const codigoDeLista = (n, msg) => ({
  mascara: (t) => (/[^\d.\s-]/.test(t) ? t : soDigitos(t)),
  valida: (v) => new RegExp(`^\\d{${n}}$`).test(v.trim()) || msg,
});
const emailValido = (e) => /^[^\s@;,]+@[^\s@;,]+\.[^\s@;,]+$/.test(e);

/**
 * Formatos dos campos marcados com data-formato: `mascara` arruma o texto ao sair do campo (aceita
 * colar com ou sem pontuação); `valida` devolve true ou a mensagem de erro. Campo vazio não passa
 * por aqui (quem decide se pode ficar vazio é o atributo required).
 */
export const FORMATOS = {
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
  ctn: codigoDeLista(6, "6 dígitos (ex.: 170101): digite o código ou escolha um na lista."),
  nbs: codigoDeLista(9, "NBS com 9 dígitos: digite o código ou escolha um na lista."),
  ctm: exatoDigitos(3, "Código municipal com 3 dígitos."),
  cst: exatoDigitos(2, "CST com 2 dígitos (ex.: 00)."),
  ind_op: codigoDeLista(6, "cIndOp com 6 dígitos (ex.: 030101): digite o código ou escolha um na lista."),
  cst_ibs: exatoDigitos(3, "CST do IBS/CBS com 3 dígitos (ex.: 000)."),
  class_trib: codigoDeLista(6, "cClassTrib com 6 dígitos (ex.: 000001): digite o código ou escolha um na lista."),
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
  const v = el.value.trim();
  if (!v) {
    if (!el.required) return "";
    if (el.tagName === "SELECT") return "Escolha uma opção.";
    if (el.dataset.arquivo !== undefined) return "Escolha o arquivo.";
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
  // campo dentro de um bloco recolhido (<details>): abre para o erro aparecer
  const recolhido = el.closest("details:not([open])");
  if (recolhido) recolhido.open = true;
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

export function limparErro(el) {
  const p = $(".erro-campo", el.closest(".grade > *") ?? el.parentElement);
  if (p) {
    const resto = (el.getAttribute("aria-describedby") ?? "").split(" ").filter((id) => id && id !== p.id);
    if (resto.length) el.setAttribute("aria-describedby", resto.join(" "));
    else el.removeAttribute("aria-describedby");
    p.remove();
  }
  el.removeAttribute("aria-invalid");
}

export function limparErros(raiz) {
  for (const el of $$('[aria-invalid="true"]', raiz)) limparErro(el);
}

/** Valida os campos visíveis de `raiz`; devolve [{ el, msg }] na ordem da tela. */
/** Visível, ou só escondido por um <details> fechado (que conta: o erro abre o bloco). */
function emUso(el) {
  if (el.getClientRects().length) return true;
  const recolhido = el.closest("details:not([open])");
  return !!recolhido && recolhido.getClientRects().length > 0 && !el.closest("[hidden]");
}

export function validarCampos(raiz) {
  return $$("input, select, textarea", raiz)
    .filter((el) => !el.disabled && el.type !== "hidden" && el.type !== "checkbox" && emUso(el))
    .map((el) => {
      aplicarMascara(el);
      return { el, msg: erroDoCampo(el) };
    })
    .filter((e) => e.msg);
}

/** Marca os campos com erro e põe o foco no primeiro; com mais de um, resume no `resumoEl`. */
export function mostrarErros(erros, resumoEl) {
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
export function confirmar(texto, { ok = "Confirmar", cancelar = "Cancelar" } = {}) {
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

export function abrirDialogo(dlg, erroEl, salvar) {
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
  if (!form || el.tagName === "BUTTON") return;
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

/** Lê um <form> como objeto (checkbox → boolean). */
export function lerForm(form) {
  const dados = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    dados[el.name] = el.type === "checkbox" ? el.checked : el.value;
  }
  return dados;
}

export function preencherForm(form, dados) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in dados)) continue;
    if (el.type === "checkbox") el.checked = !!dados[el.name];
    else {
      el.value = dados[el.name] ?? "";
      if (el.dataset.formato) aplicarMascara(el);
    }
  }
}
