// Atualização do sistema: procura, baixa e instala versões novas.

import { $, $$, esc, invoke, toast } from "./util.js";
import { confirmar } from "./formularios.js";
import { emissaoEmCurso } from "./emitir.js";
import { cancelamentoEmCurso } from "./notas.js";
import { loteEmAndamento } from "./lote.js";
import { planilhaEmAndamento } from "./planilha.js";

let atualizacao = null; // { versao, notas } da Release mais nova, se houver

function mostrarSituacaoAtualizacao(texto) {
  $("#situacao-atualizacao").textContent = texto;
  $("#instalar-atualizacao").hidden = !atualizacao;
}

/** Consulta a Release mais nova. `silencioso`: na abertura, sem internet não é erro. */
export async function procurarAtualizacao(silencioso) {
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
export let atualizando = false;

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
