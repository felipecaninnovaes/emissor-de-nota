// Serviços: cadastro, importação e exportação.

import {
  $,
  $$,
  chamar,
  esc,
  estado,
  invoke,
  moeda,
  percentualTela,
  soDigitos,
  toast,
  valorDigitado,
  vazio,
} from "./util.js";
import { acoesLinha, linhaVazia, selecaoEmTabela } from "./tabela.js";
import { ErroCampo, abrirDialogo, confirmar, preencherForm } from "./formularios.js";
import { ligarCombobox } from "./componentes.js";
import { renderizarServicosEmitir } from "./emitir.js";

const NOME_RETENCAO = { irrf: "IRRF", csll: "CSLL", cp: "INSS", pis: "PIS", cofins: "COFINS" };

export async function carregarServicos() {
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
