// Importar planilha: lê, confere e emite notas a partir de uma planilha.

import { $, $$, NOME_SITUACAO, chamar, esc, estado, invoke, moeda, somarValores, toast } from "./util.js";
import { detalheLinha, selecaoEmTabela } from "./tabela.js";
import { ambienteDe, celulaEmpresa, confirmarEmissao, emitirTravando } from "./lote.js";
import { editarTomador } from "./tomadores.js";
import { FILTRO_PLANILHA, ligarEscolhaArquivo } from "./componentes.js";

let planilhaLida = [];
let planilhaArquivo = null; // { nome, bytes }
export let planilhaEmAndamento = false;

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

ligarEscolhaArquivo($("#arquivo-planilha"), $("#escolher-planilha"), { titulo: "Escolher planilha", ...FILTRO_PLANILHA }, (a) => {
  planilhaArquivo = a;
  return lerPlanilha();
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

export function desenharPlanilha() {
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
  const controles = ["#mes-planilha", "#reler-planilha", "#escolher-planilha"];
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
