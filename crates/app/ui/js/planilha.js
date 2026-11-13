// Importar planilha: lê, confere e emite notas a partir de uma planilha ou de um e-mail com
// pedidos de nota (.eml, .msg ou texto colado; spec 0001).

import { $, $$, NOME_SITUACAO, chamar, esc, estado, invoke, moeda, somarValores, toast } from "./util.js";
import { detalheLinha, selecaoEmTabela } from "./tabela.js";
import { ambienteDe, celulaEmpresa, confirmarEmissao, emitirTravando } from "./lote.js";
import { editarTomador } from "./tomadores.js";
import { FILTRO_PLANILHA_EMAIL, ligarEscolhaArquivo } from "./componentes.js";

let planilhaLida = [];
let planilhaArquivo = null; // { id, nome, bytes }
// Texto colado lido por último (no lugar do arquivo); "Ler de novo" relê o campo.
let textoColado = null;
const TEXTO_COLADO = "texto colado";
const eEmail = (nome) => /\.(eml|msg)$/i.test(nome ?? "");
export let planilhaEmAndamento = false;
// Número da leitura mais recente: a do e-mail demora (consulta de CEP), e uma leitura antiga que
// termina depois (o mês ou o serviço mudou no meio) não pode sobrescrever a tabela.
let leituraAtual = 0;
// Leitura em curso: as linhas na tela são as da leitura anterior e não podem ser emitidas.
let lendo = false;

const podeEmitirDaPlanilha = (it) => !it.erros.length && (!it.anterior || it.anterior.libera);
const selecaoPlanilha = selecaoEmTabela({
  lista: "#lista-planilha",
  itens: () => planilhaLida,
  botao: "#planilha-emitir",
  rotulo: "Emitir selecionadas",
  todos: "#planilha-todos",
  marcar: "#planilha-marcar",
  emAndamento: () => planilhaEmAndamento || lendo,
});

$("#modelo-planilha").addEventListener("click", async () => {
  try {
    toast(`Modelo salvo em ${await chamar("salvar_modelo_planilha")}`);
  } catch (e) {
    toast(String(e), true);
  }
});

ligarEscolhaArquivo(
  $("#arquivo-planilha"),
  $("#escolher-planilha"),
  { titulo: "Escolher planilha ou e-mail", ...FILTRO_PLANILHA_EMAIL },
  (a) => {
    planilhaArquivo = a;
    textoColado = null;
    return lerPlanilha();
  },
);

const temOrigem = () => planilhaArquivo || textoColado !== null;
$("#mes-planilha").addEventListener("change", () => temOrigem() && lerPlanilha());
// O serviço só vale para o e-mail; a planilha traz o dela na coluna.
$("#servico-planilha").addEventListener("change", () => (textoColado !== null || eEmail(planilhaArquivo?.nome)) && lerPlanilha());

$("#texto-email").addEventListener("input", () => {
  $("#ler-texto-email").disabled = !$("#texto-email").value.trim() || planilhaEmAndamento;
});
$("#ler-texto-email").addEventListener("click", () => {
  const texto = $("#texto-email").value;
  if (!texto.trim() || planilhaEmAndamento) return;
  textoColado = texto;
  planilhaArquivo = null;
  $("#arquivo-planilha").value = "";
  return lerPlanilha();
});

/** Serviços da empresa para o e-mail; com um só, já vem escolhido. */
export function renderizarServicosPlanilha() {
  const sel = $("#servico-planilha");
  const anterior = sel.value;
  const servicos = estado.servicos ?? [];
  sel.innerHTML =
    (servicos.length === 1 ? "" : `<option value="">Escolha o serviço</option>`) +
    servicos.map((s) => `<option value="${s.id}">${esc(s.nome)}</option>`).join("");
  if (servicos.some((s) => String(s.id) === anterior)) sel.value = anterior;
}
// Abre o arquivo de novo do disco: o operador pode ter corrigido a planilha depois de escolhê-la.
$("#reler-planilha").addEventListener("click", async () => {
  if (!temOrigem() || planilhaEmAndamento) return;
  if (textoColado !== null) {
    // O texto colado é relido do campo: o operador pode tê-lo corrigido.
    textoColado = $("#texto-email").value;
    return lerPlanilha();
  }
  try {
    planilhaArquivo = await chamar("reler_arquivo_escolhido", { id: planilhaArquivo.id });
  } catch (e) {
    toast(String(e), true);
    return;
  }
  await lerPlanilha();
});

async function lerPlanilha() {
  // Durante a emissão a lista mostra o andamento; relê ao terminar.
  if (planilhaEmAndamento) return;
  const leitura = ++leituraAtual;
  $("#planilha-andamento").innerHTML = "";
  lendo = true;
  selecaoPlanilha.atualizar();
  let lidas;
  try {
    lidas =
      textoColado !== null || eEmail(planilhaArquivo?.nome) ? await lerEmail() : await chamar("ler_planilha", {
        nome: planilhaArquivo.nome,
        bytes: planilhaArquivo.bytes,
        empresaId: estado.empresa?.id ?? null,
        mes: $("#mes-planilha").value,
      });
  } catch (e) {
    if (leitura !== leituraAtual) return;
    lendo = false;
    lidas = [];
    $("#planilha-andamento").innerHTML = `<div class="aviso erro">${esc(e)}</div>`;
  }
  if (leitura !== leituraAtual) return;
  lendo = false;
  planilhaLida = lidas;
  desenharPlanilha();
}

/** Lê o e-mail (arquivo ou texto colado), mostrando o andamento das consultas de CEP. */
async function lerEmail() {
  const leitura = leituraAtual;
  const andamento = $("#planilha-andamento");
  const parar = await window.__TAURI__.event.listen("andamento-email", ({ payload }) => {
    if (leitura !== leituraAtual) return;
    andamento.innerHTML =
      payload.feito < payload.total
        ? `<div class="aviso">Consultando CEP ${payload.feito + 1} de ${payload.total}…</div>`
        : "";
  });
  try {
    const servico = $("#servico-planilha").value;
    return await chamar("ler_email", {
      nome: textoColado === null ? planilhaArquivo.nome : null,
      bytes: textoColado === null ? planilhaArquivo.bytes : null,
      textoColado,
      empresaId: estado.empresa?.id ?? null,
      servicoId: servico ? Number(servico) : null,
      mes: $("#mes-planilha").value,
    });
  } finally {
    parar();
    if (leitura === leituraAtual) andamento.innerHTML = "";
  }
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
  const aviso = it.tomador_novo
    ? `<p>O tomador será cadastrado nesta empresa ao emitir.</p>`
    : it.vincular_tomador
      ? `<p>O tomador será vinculado a esta empresa.</p>`
      : "";
  const avisos = it.avisos?.length ? `<p>${it.avisos.map(esc).join("<br>")}</p>` : "";
  const antes = t ? `<p>Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</p>` : "";
  return {
    selo: `<span class="selo ${avisos ? "incerta" : "pendente"}">${avisos ? "Pronta, confira" : "Pronta"}</span>`,
    motivo: avisos + aviso + antes,
    avisos: !!avisos,
  };
}

export function desenharPlanilha() {
  const corpo = $("#lista-planilha");
  $("#planilha-todos").checked = false;
  if (!planilhaLida.length) {
    corpo.innerHTML = `<tr><td colspan="8" class="vazio">${
      temOrigem()
        ? "Nenhuma linha para emitir."
        : "Escolha a planilha (.xlsx, .xls, .ods ou .csv) ou o e-mail (.eml ou .msg), ou cole o texto do e-mail."
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
        aberta: !!s.erro || !!s.avisos,
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
  const controles = ["#mes-planilha", "#reler-planilha", "#escolher-planilha", "#servico-planilha", "#ler-texto-email"];
  const resumo = await emitirTravando(controles, andamento, {
    itens,
    andamento: $("#planilha-andamento"),
    celulaDe: (it) => $(`#lista-planilha tr[data-i="${planilhaLida.indexOf(it)}"] [data-situacao]`),
    descrever: (it) => `linha ${it.linha}: ${it.tomador ?? it.documento_tomador}`,
    emitirUm: (it) => invoke("emitir_planilha", { item: it, arquivo: textoColado !== null ? TEXTO_COLADO : (planilhaArquivo?.nome ?? null) }),
  });
  await lerPlanilha();
  // A trava reabilita os controles; o botão do texto só vale com texto no campo.
  $("#ler-texto-email").disabled = !$("#texto-email").value.trim();
  $("#planilha-andamento").innerHTML = resumo;
});
