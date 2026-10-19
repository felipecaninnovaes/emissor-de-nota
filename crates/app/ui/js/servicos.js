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
import { ErroCampo, abrirDialogo, confirmar, limparErro, preencherForm } from "./formularios.js";
import { FILTRO_PLANILHA, ligarCombobox, ligarEscolhaArquivo } from "./componentes.js";
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

const pontuarCtn = (c) => `${c.slice(0, 2)}.${c.slice(2, 4)}.${c.slice(4)}`;
const pontuarNbs = (c) => `${c.slice(0, 1)}.${c.slice(1, 5)}.${c.slice(5, 7)}.${c.slice(7)}`;

/**
 * Campo de código com busca na tabela oficial embutida (Lista de Serviços Nacional ou NBS): o
 * operador digita os dígitos ou parte do nome e escolhe na lista; a descrição oficial do código
 * aparece abaixo do campo. Devolve a função que atualiza essa descrição (usada ao abrir o diálogo).
 * `detalhar(item)` troca o texto da descrição e `aoEscolher(item)` roda ao escolher na lista ou ao
 * digitar um código da tabela (o diálogo ao abrir não dispara: o que está gravado não é alterado).
 */
function ligarCampoDeCodigo({
  campo,
  caixa,
  ajuda,
  digitos,
  comando,
  nomeLista,
  pontuar,
  rotulo,
  ajudaPadrao,
  detalhar = (i) => i.descricao,
  aoEscolher: aoEscolherItem,
}) {
  async function descrever(aoDigitar = false) {
    const codigo = soDigitos(campo.value);
    if (codigo.length !== digitos) {
      ajuda.textContent = ajudaPadrao;
      return;
    }
    try {
      const { fonte, itens } = await invoke(comando, { termo: codigo });
      if (soDigitos(campo.value) !== codigo) return; // o usuário já mudou o código
      if (aoDigitar && itens.length) aoEscolherItem?.(itens[0]);
      ajuda.textContent = itens.length
        ? `${pontuar(codigo)} · ${detalhar(itens[0])}`
        : `O código ${codigo} não está na ${nomeLista} embutida (${fonte}); confira antes de emitir.`;
    } catch (e) {
      toast(String(e), true);
    }
  }

  campo.addEventListener("input", () => descrever(true));
  ligarCombobox(campo, caixa, {
    rotulo,
    vazio: `Nenhum código da ${nomeLista} encontrado.`,
    contagem: (n) => `${n} código(s) encontrado(s).`,
    buscar: async (termo) => (await invoke(comando, { termo })).itens,
    opcao: (i) => `${esc(i.descricao)}<small>${pontuar(i.codigo)}${i.grupo ? ` · ${esc(i.grupo)}` : ""}</small>`,
    aoEscolher(i) {
      campo.value = i.codigo;
      caixa.hidden = true;
      // o código escolhido é válido: tira a marca de erro, se havia
      if (campo.getAttribute("aria-invalid") === "true") limparErro(campo);
      aoEscolherItem?.(i);
      descrever();
    },
  });
  return descrever;
}

const descreverCtn = ligarCampoDeCodigo({
  campo: $("#servico-codigo-trib-nac"),
  caixa: $("#servico-resultados-ctn"),
  ajuda: $("#servico-ctn-descricao"),
  digitos: 6,
  comando: "buscar_lista_servicos",
  nomeLista: "Lista de Serviços Nacional",
  pontuar: pontuarCtn,
  rotulo: "Códigos de serviço",
  ajudaPadrao: "Digite os 6 dígitos ou parte do nome para buscar na Lista de Serviços Nacional.",
});

const descreverNbs = ligarCampoDeCodigo({
  campo: $("#servico-nbs"),
  caixa: $("#servico-resultados-nbs"),
  ajuda: $("#servico-nbs-descricao"),
  digitos: 9,
  comando: "buscar_nbs",
  nomeLista: "NBS",
  pontuar: pontuarNbs,
  rotulo: "Códigos NBS",
  ajudaPadrao: "Opcional. Digite os 9 dígitos ou parte do nome para buscar na NBS.",
});

const descreverIndOp = ligarCampoDeCodigo({
  campo: $("#servico-ibs-ind-op"),
  caixa: $("#servico-resultados-ind-op"),
  ajuda: $("#servico-ind-op-descricao"),
  digitos: 6,
  comando: "buscar_ind_operacao",
  nomeLista: "tabela de indicadores da operação",
  pontuar: (c) => c,
  rotulo: "Indicadores da operação",
  ajudaPadrao: "Digite os 6 dígitos ou parte do nome (ex.: presencial) para buscar na tabela oficial.",
  detalhar: (i) => `${i.descricao}. Local da operação: ${i.grupo}.`,
});

const pontuarClass = (c) => c;
/** Redução de alíquota do cClassTrib, em texto curto ("redução de 60% no IBS e na CBS"). */
function textoReducao(i) {
  const [ibs, cbs] = [i.reducao_ibs, i.reducao_cbs];
  if (ibs === "0" && cbs === "0") return "sem redução de alíquota";
  return ibs === cbs ? `redução de ${ibs}% no IBS e na CBS` : `redução de ${ibs}% no IBS e ${cbs}% na CBS`;
}

const descreverClass = ligarCampoDeCodigo({
  campo: $("#servico-ibs-class"),
  caixa: $("#servico-resultados-class"),
  ajuda: $("#servico-class-descricao"),
  digitos: 6,
  comando: "buscar_class_trib",
  nomeLista: "classificação tributária",
  pontuar: pontuarClass,
  rotulo: "Classificações tributárias",
  ajudaPadrao: "Digite os 6 dígitos ou parte do nome (ex.: saúde) para buscar na tabela oficial do IBS/CBS.",
  detalhar: (i) =>
    `${i.descricao} CST ${i.cst} (${i.grupo}), ${textoReducao(i)}.` +
    (i.fim_vigencia ? ` Vigência encerrada em ${i.fim_vigencia}.` : ""),
  // cada cClassTrib tem um só CST: o campo ao lado acompanha o código escolhido
  aoEscolher(i) {
    const cst = $("#servico-ibs-cst");
    cst.value = i.cst;
    if (cst.getAttribute("aria-invalid") === "true") limparErro(cst);
  },
});

/** Percentual digitado ("11,48") → texto aceito pelo backend, ou null. */
function percentualDigitado(t) {
  const v = vazio(t);
  return v == null ? null : v.replace(",", ".");
}

/** Alíquotas de PIS e COFINS de apuração própria preenchidas no cadastro (só as informadas). */
function aliquotasProprias(d) {
  const campos = {};
  const pis = percentualDigitado(d.aliq_pis);
  const cofins = percentualDigitado(d.aliq_cofins);
  if (pis != null) campos.aliquota_pis = pis;
  if (cofins != null) campos.aliquota_cofins = cofins;
  return campos;
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
      aliq_pis: percentualTela(pc.aliquota_pis),
      aliq_cofins: percentualTela(pc.aliquota_cofins),
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
  descreverNbs();
  descreverClass();
  descreverIndOp();
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
            indicador_operacao: soDigitos(d.ibs_ind_op),
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
          pis_cofins: {
            cst: d.cst.trim() || "00",
            tipo_retencao: Number(d.tp_ret),
            ...aliquotasProprias(d),
          },
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

let planilhaServicos = null; // { nome, bytes } da planilha escolhida na importação

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
  // O campo volta vazio no reset do diálogo; o conteúdo guardado só vale com o nome à mostra.
  const arquivo = $("#importar-arquivo").value && planilhaServicos;
  if (!arquivo) return desenharServicosAImportar("Escolha a planilha (.xlsx, .xls, .ods ou .csv).");
  try {
    servicosAImportar = await chamar("ler_planilha_servicos", { nome: arquivo.nome, bytes: arquivo.bytes });
  } catch (e) {
    return desenharServicosAImportar(String(e));
  }
  desenharServicosAImportar("Nenhuma linha com dados na planilha.");
}

$("#importar-origem").addEventListener("change", carregarOrigemImportacao);
$("#importar-empresa").addEventListener("change", carregarOrigemImportacao);
ligarEscolhaArquivo(
  $("#importar-arquivo"),
  $("#escolher-importar-arquivo"),
  { titulo: "Escolher planilha de serviços", ...FILTRO_PLANILHA },
  (a) => {
    planilhaServicos = a;
    return carregarOrigemImportacao();
  },
);

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
