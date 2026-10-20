// Navegação entre telas, empresa atual e menu lateral recolhível.

import { $, $$, NOME_AMBIENTE, esc, estado, invoke, toast } from "./util.js";
import { descartarRevisao, limparEmissao, renderizarServicosEmitir } from "./emitir.js";
import { carregarNotas } from "./notas.js";
import { carregarRecorrencias } from "./recorrentes.js";
import { carregarLote } from "./lote.js";
import { desenharPlanilha } from "./planilha.js";
import { carregarTomadores } from "./tomadores.js";
import { carregarServicos } from "./servicos.js";
import { renderizarEmpresas } from "./empresas.js";
import { carregarConfiguracoes } from "./configuracoes.js";

/** Mostra uma tela; `focar` leva o foco ao título dela (troca feita pelo operador no menu). */
export function irPara(tela, focar = false) {
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
export function atualizarMenu() {
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

export async function carregarEmpresas() {
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

export function lerPreferencia(chave) {
  try {
    return localStorage.getItem("emissor." + chave);
  } catch {
    return null;
  }
}

export function gravarPreferencia(chave, valor) {
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

/** Empresa para a qual o formulário de emissão foi preparado (ver `trocarEmpresa`). */
let empresaDaEmissao;

async function trocarEmpresa(empresa) {
  if (empresa?.id !== estado.empresa?.id) descartarRevisao();
  estado.empresa = empresa;
  const selo = $("#selo-ambiente");
  const info = $("#info-certificado");
  $("#faixa-teste").hidden = empresa?.ambiente !== "homologacao";
  if (!empresa) {
    empresaDaEmissao = undefined;
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
  // Outra empresa: formulário limpo. A mesma recarregada (cadastro, certificado ou numeração
  // corrigidos, às vezes a partir de uma rejeição) mantém o que foi digitado; só a revisão é refeita.
  if (empresa.id !== empresaDaEmissao) {
    limparEmissao();
    empresaDaEmissao = empresa.id;
  } else {
    descartarRevisao();
  }
  estado.servicos = await invoke("listar_servicos", { empresaId: empresa.id });
  renderizarServicosEmitir();
  // Recarrega a tela visível com a nova empresa.
  const visivel = $$("section[data-tela]").find((s) => !s.hidden)?.dataset.tela;
  if (visivel && visivel !== "emitir" && visivel !== "empresas") irPara(visivel);
}
