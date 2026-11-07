// Tomadores: cadastro, e-mails e retenções por empresa, e importação por planilha.

import { $, $$, chamar, esc, estado, formatarDocumento, invoke, soDigitos, toast, vazio } from "./util.js";
import { acoesLinha, linhaVazia, selecaoEmTabela } from "./tabela.js";
import { ErroCampo, abrirDialogo, preencherForm } from "./formularios.js";
import { FILTRO_PLANILHA, ligarEscolhaArquivo, vincularTomador } from "./componentes.js";

let tomadoresCarregados = [];

const POR_PAGINA = 25;
let pagina = 0; // contada de 0
let empresaDaLista = null; // trocar de empresa volta para a primeira página

export async function carregarTomadores() {
  const corpo = $("#lista-tomadores");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(5, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    desenharPaginacao(0);
    return;
  }
  if (empresaDaLista !== estado.empresa.id) pagina = 0;
  empresaDaLista = estado.empresa.id;
  const termo = $("#busca-tomadores").value.trim();
  const listar = () =>
    invoke("listar_tomadores", { empresaId: estado.empresa.id, termo, pagina, porPagina: POR_PAGINA });
  let resultado = await listar();
  // A página pode ter sumido (busca mais curta, tomador removido): volta para a última que existe.
  if (!resultado.itens.length && pagina > 0) {
    pagina = Math.max(0, Math.ceil(resultado.total / POR_PAGINA) - 1);
    resultado = await listar();
  }
  tomadoresCarregados = resultado.itens;
  // Os de outras empresas (até 20) vêm só na primeira página, depois dos desta.
  if (termo && pagina === 0) {
    const outros = await invoke("buscar_tomadores_de_outras_empresas", { empresaId: estado.empresa.id, termo });
    tomadoresCarregados.push(...outros.map((t) => ({ ...t, de_outra_empresa: true })));
  }
  desenharPaginacao(resultado.total);
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
    : linhaVazia(5, "Nenhum tomador encontrado.", {
        clicar: "#novo-tomador",
        texto: "Novo tomador",
      });
}

/** "41 a 50 de 50 tomadores" e os botões de página; some com uma página só. */
function desenharPaginacao(total) {
  const paginas = Math.ceil(total / POR_PAGINA);
  $("#paginacao-tomadores").hidden = paginas <= 1;
  const inicio = pagina * POR_PAGINA;
  $("#tomadores-faixa").textContent =
    `${inicio + 1} a ${Math.min(inicio + POR_PAGINA, total)} de ${total} tomadores · página ${pagina + 1} de ${paginas}`;
  $("#tomadores-anterior").disabled = pagina === 0;
  $("#tomadores-proxima").disabled = pagina >= paginas - 1;
}

for (const [id, passo] of [
  ["#tomadores-anterior", -1],
  ["#tomadores-proxima", 1],
]) {
  $(id).addEventListener("click", () => {
    pagina = Math.max(0, pagina + passo);
    carregarTomadores().catch((e) => toast(String(e), true));
  });
}

let buscaTomadores = 0;
$("#busca-tomadores").addEventListener("input", () => {
  pagina = 0;
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
export function editarTomador(dto, depois, { empresaId = estado.empresa?.id, documento = null } = {}) {
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
      tipo_tomador: dto.tipo,
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
    const tipo = d.tipo_tomador || "privado";
    const pessoaId = await chamar("salvar_tomador", { empresaId, tomador, emails, retemFederais, retemIss, tipo });
    toast("Tomador salvo.");
    depois?.({ pessoa_id: pessoaId, tomador, emails, retem_federais: retemFederais, retem_iss: retemIss, tipo });
  });
  if (!dto && documento) {
    // Vindo da planilha: já preenche o documento e, se for CNPJ, consulta os dados.
    const cpf = documento.length === 11;
    form.elements.tipo.value = cpf ? "cpf" : "cnpj";
    form.elements.documento.value = formatarDocumento(cpf ? { cpf: documento } : { cnpj: documento });
    form.elements.documento.dispatchEvent(new Event("change"));
  }
}

// ---------------------------------------------------------------- importar tomadores

/**
 * Linhas lidas da planilha: [{ linha, documento, nome, item, erros, avisos, a_completar, ... }];
 * `item` vazio quando há erro. A linha volta inteira ao backend para ser completada.
 */
let tomadoresAImportar = [];

/** Muda a cada leitura e ao fechar o diálogo: a consulta de CNPJ em andamento para. */
let geracaoConsulta = 0;
let consultandoCnpj = false;

const completarPorCnpj = () => $("#importar-tomadores-consultar").checked;

function linhaTomadorAImportar(it, i) {
  const ok = !!it.item && !it.erros.length;
  const aguarda = it.a_completar && completarPorCnpj();
  const avisos = it.avisos?.length ? `<span class="ajuda"> ${esc(it.avisos.join("; "))}</span>` : "";
  const situacao = aguarda
    ? `<span class="selo pendente">Consultar CNPJ</span>`
    : ok
      ? `<span class="selo autorizada">Pronto</span>${avisos}`
      : `<span class="selo rejeitada" title="${esc(it.erros.join("\n"))}">${esc(it.erros.join("; "))}</span>${avisos}`;
  const documento = it.item ? formatarDocumento(it.item.tomador.documento) : it.documento;
  const alvo = `linha ${it.linha}: ${it.nome || documento}`;
  return `<tr>
    <td><input type="checkbox" data-i="${i}" ${ok ? "checked" : "disabled"} aria-label="${esc(`Marcar ${alvo}`)}"></td>
    <td class="dado">${esc(documento)}</td>
    <td title="${esc(alvo)}">${esc(it.item?.tomador.nome || it.nome || "(sem nome)")}</td>
    <td class="sec-1">${situacao}</td>
  </tr>`;
}

function desenharTomadoresAImportar(vazioTexto) {
  const corpo = $("#lista-importar-tomadores");
  $("#importar-tomadores-todos").checked = false;
  corpo.innerHTML = tomadoresAImportar.length
    ? tomadoresAImportar.map(linhaTomadorAImportar).join("")
    : `<tr><td colspan="4" class="vazio">${esc(vazioTexto)}</td></tr>`;
  selecaoImportarTomadores.atualizar();
}

/**
 * Consulta o CNPJ das linhas sem nome ou sem endereço, uma por vez (a BrasilAPI limita as
 * consultas), e troca cada linha pela completada sem mexer nas marcações das outras.
 */
async function consultarCnpjsDaPlanilha() {
  const geracao = ++geracaoConsulta;
  const pendentes = tomadoresAImportar.flatMap((it, i) => (it.a_completar ? [i] : []));
  const andamento = $("#importar-tomadores-andamento");
  if (!pendentes.length || !completarPorCnpj()) {
    andamento.hidden = true;
    return;
  }
  consultandoCnpj = true;
  andamento.hidden = false;
  try {
    for (const [n, i] of pendentes.entries()) {
      andamento.textContent = `Consultando CNPJ ${n + 1} de ${pendentes.length}...`;
      const linha = await chamar("completar_tomador_importado", { linha: tomadoresAImportar[i] });
      if (geracao !== geracaoConsulta) return;
      tomadoresAImportar[i] = linha;
      const tr = $("#lista-importar-tomadores").rows[i];
      if (tr) tr.outerHTML = linhaTomadorAImportar(linha, i);
      selecaoImportarTomadores.atualizar();
    }
    andamento.textContent = `Consulta de CNPJ concluída (${pendentes.length}). Confira os dados antes de importar.`;
  } catch (e) {
    if (geracao === geracaoConsulta) andamento.textContent = `Consulta de CNPJ interrompida: ${e}`;
  } finally {
    if (geracao === geracaoConsulta) {
      consultandoCnpj = false;
      // As que não chegaram a ser consultadas voltam a mostrar o erro ou "Pronto".
      for (const it of tomadoresAImportar) it.a_completar = false;
      desenharMantendoMarcas();
    }
  }
}

/** Redesenha a tabela sem perder o que o operador desmarcou. */
function desenharMantendoMarcas() {
  const desmarcadas = new Set(
    $$("#lista-importar-tomadores input[type=checkbox]")
      .filter((c) => !c.checked && !c.disabled)
      .map((c) => c.dataset.i),
  );
  desenharTomadoresAImportar("");
  for (const c of $$("#lista-importar-tomadores input[type=checkbox]")) if (desmarcadas.has(c.dataset.i)) c.checked = false;
  selecaoImportarTomadores.atualizar();
}

$("#importar-tomadores-consultar").addEventListener("change", () => {
  if (completarPorCnpj()) {
    consultarCnpjsDaPlanilha();
  } else {
    geracaoConsulta++;
    consultandoCnpj = false;
    $("#importar-tomadores-andamento").hidden = true;
    desenharMantendoMarcas();
  }
});

$("#dlg-importar-tomadores").addEventListener("close", () => {
  geracaoConsulta++;
  consultandoCnpj = false;
});

const selecaoImportarTomadores = selecaoEmTabela({
  lista: "#lista-importar-tomadores",
  itens: () => tomadoresAImportar,
  botao: "#importar-tomadores-ok",
  rotulo: "Importar",
  todos: "#importar-tomadores-todos",
});

let planilhaTomadores = null; // { nome, bytes } da planilha escolhida

async function lerPlanilhaTomadores() {
  geracaoConsulta++;
  consultandoCnpj = false;
  $("#importar-tomadores-andamento").hidden = true;
  tomadoresAImportar = [];
  // O campo volta vazio no reset do diálogo; o conteúdo guardado só vale com o nome à mostra.
  const arquivo = $("#importar-tomadores-arquivo").value && planilhaTomadores;
  if (!arquivo) return desenharTomadoresAImportar("Escolha a planilha (.xlsx, .xls, .ods ou .csv).");
  try {
    tomadoresAImportar = await chamar("ler_planilha_tomadores", { nome: arquivo.nome, bytes: arquivo.bytes });
  } catch (e) {
    return desenharTomadoresAImportar(String(e));
  }
  desenharTomadoresAImportar("Nenhuma linha com dados na planilha.");
  consultarCnpjsDaPlanilha();
}

ligarEscolhaArquivo(
  $("#importar-tomadores-arquivo"),
  $("#escolher-importar-tomadores"),
  { titulo: "Escolher planilha de tomadores", ...FILTRO_PLANILHA },
  (a) => {
    planilhaTomadores = a;
    return lerPlanilhaTomadores();
  },
);

$("#modelo-tomadores").addEventListener("click", async () => {
  try {
    toast(`Modelo salvo em ${await chamar("salvar_modelo_tomadores")}`);
  } catch (e) {
    toast(String(e), true);
  }
});

$("#importar-tomadores").addEventListener("click", () => {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  const empresaId = estado.empresa.id;
  const dlg = $("#dlg-importar-tomadores");
  $("form", dlg).reset();
  lerPlanilhaTomadores();
  abrirDialogo(dlg, $("#erro-importar-tomadores"), async () => {
    if (consultandoCnpj) throw "Aguarde a consulta de CNPJ terminar (ou desmarque a opção de completar).";
    const itens = selecaoImportarTomadores.marcados().map((it) => it.item);
    if (!itens.length) throw "Marque os tomadores a importar.";
    const r = await chamar("importar_tomadores", { empresaId, itens });
    toast(`Tomadores: ${r.novos} novo(s), ${r.atualizados} atualizado(s).`);
    return carregarTomadores;
  });
});
