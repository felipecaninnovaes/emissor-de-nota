// Atalhos de teclado, preferências do dia a dia e aparência (tamanho do texto e densidade).

import { $, $$, invoke, mesAtual, toast } from "./util.js";
import { atualizarMenu, gravarPreferencia, irPara, lerPreferencia } from "./navegacao.js";
import { limparEmissao } from "./emitir.js";

export const TELAS = $$("nav button[data-tela]").map((b) => b.dataset.tela);
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
export function mesLembrado(chave, padrao) {
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

// ---------------------------------------------------------------- aparência (tamanho do texto e densidade)

const ZOOMS = ["0.9", "1", "1.1", "1.25", "1.4", "1.5"];
export let aparencia = { zoom: "1", densidade: "auto" };

/** Lê a aparência gravada e aplica (na abertura; falha mantém o padrão). */
export function carregarAparencia() {
  invoke("obter_aparencia")
    .then((a) => {
      aparencia = a;
      aplicarAparencia();
    })
    .catch(() => {});
}

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
