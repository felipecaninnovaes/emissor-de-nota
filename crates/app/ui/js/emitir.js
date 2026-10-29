// Tela de emissão: tomador, serviço, obra, revisão e envio.

import {
  $,
  $$,
  chamar,
  esc,
  estado,
  formatarDocumento,
  invoke,
  mesPadrao,
  moeda,
  soDigitos,
  toast,
  valorDigitado,
  vazio,
} from "./util.js";
import { ErroCampo, FORMATOS, limparErros, mostrarErros, validarCampos } from "./formularios.js";
import { anunciar, ligarBuscaTomador, mostrarTomadorEscolhido } from "./componentes.js";
import { enviarEmailNota } from "./notas.js";
import { editarRecorrencia } from "./recorrentes.js";
import { editarTomador } from "./tomadores.js";
import { atualizando } from "./atualizacao.js";
import { irPara } from "./navegacao.js";
import { editarEmpresa, editarNumeracao, importarCertificado } from "./empresas.js";
import { editarServico } from "./servicos.js";

/** Descrição que `aplicarServico` pôs no campo; diferente do campo = o operador mudou o texto. */
let descricaoAplicada = null;

export function renderizarServicosEmitir() {
  const sel = $("#servico");
  const anterior = sel.value;
  sel.innerHTML = estado.servicos.length
    ? estado.servicos.map((s) => `<option value="${s.id}">${esc(s.nome)}</option>`).join("")
    : `<option value="">Cadastre um serviço em "Serviços"</option>`;
  const mesmo = estado.servicos.find((s) => String(s.id) === anterior);
  if (mesmo) {
    // Lista recarregada com o mesmo serviço (cadastro editado): o que foi digitado fica; a
    // descrição só acompanha o cadastro se o operador não a mudou.
    sel.value = anterior;
    descartarRevisao(); // a revisão aberta mostra o cadastro antigo
    if ($("#descricao").value === descricaoAplicada) $("#descricao").value = descricaoAplicada = mesmo.descricao;
    $("#bloco-obra").hidden = !CODIGOS_COM_OBRA.includes(mesmo.codigo_trib_nac);
  } else {
    aplicarServico();
  }
  atualizarPrimeirosPassos();
}

export function aplicarServico() {
  const s = estado.servicos.find((x) => x.id === Number($("#servico").value));
  if (!s) return;
  $("#descricao").value = descricaoAplicada = s.descricao;
  if (s.valor_sugerido) $("#valor").value = moeda(s.valor_sugerido);
  $("#bloco-obra").hidden = !CODIGOS_COM_OBRA.includes(s.codigo_trib_nac);
}

// ---- primeiros passos: o que falta para a primeira nota, na ordem, com o botão que resolve

const PASSOS = [
  {
    feito: () => estado.empresas.length > 0,
    texto: "Cadastre a empresa que vai emitir as notas.",
    botao: "Cadastrar empresa",
    fazer: () => {
      irPara("empresas");
      editarEmpresa(null);
    },
  },
  {
    feito: () => !!estado.empresa?.certificado,
    texto: "Importe o certificado digital A1 da empresa (arquivo .pfx ou .p12 e a senha dele).",
    botao: "Importar certificado",
    fazer: () => importarCertificado(estado.empresa),
  },
  {
    feito: () => !!estado.empresa && !estado.empresa.conferir_numeracao,
    texto: "Confira na Sefin a numeração das notas (outro emissor ou outra máquina pode ter usado a série).",
    botao: "Conferir numeração",
    fazer: () => editarNumeracao(estado.empresa),
  },
  {
    feito: () => !!estado.empresa && estado.servicos.length > 0,
    texto: "Cadastre o serviço que a empresa presta: código, impostos e o texto da nota.",
    botao: "Cadastrar serviço",
    fazer: () => {
      irPara("servicos");
      editarServico(null);
    },
  },
];

/** Mostra a lista enquanto falta algum passo; só o primeiro pendente tem botão. */
export function atualizarPrimeirosPassos() {
  const atual = PASSOS.findIndex((p) => !p.feito());
  $("#primeiros-passos").hidden = atual < 0;
  if (atual < 0) return;
  $("#lista-passos").innerHTML = PASSOS.map((p, i) => {
    const classe = p.feito() ? "feito" : i === atual ? "atual" : "depois";
    const botao = i === atual ? `<button type="button" class="botao primario pequeno" data-passo="${i}">${esc(p.botao)}</button>` : "";
    return `<li class="${classe}"><span>${esc(p.texto)}</span>${botao}</li>`;
  }).join("");
}

$("#lista-passos").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-passo]");
  if (b) PASSOS[Number(b.dataset.passo)].fazer();
});

$("#servico").addEventListener("change", aplicarServico);

// ---- dados da obra (construção civil): mesma lista do nfse-core (regra E0370 da Sefin)

export const CODIGOS_COM_OBRA = [
  "070201", "070202", "070401", "070501", "070502", "070601", "070602", "070701", "070801", "071701",
  "071901", "141403", "141404",
];

// O bloco da obra existe na emissão (ids `obra-*`) e no diálogo da recorrência (`rec-obra-*`).

function ajustarTipoObra(p = "obra") {
  const tipo = $(`#${p}-tipo`).value;
  $(`#${p}-campo-codigo`).hidden = tipo === "endereco";
  $(`#${p}-endereco`).hidden = tipo !== "endereco";
  $(`#${p}-codigo-rotulo`).textContent = tipo === "cib" ? "Código CIB (8 caracteres)" : "Número do CNO / CEI";
  $(`#${p}-codigo`).maxLength = tipo === "cib" ? 8 : 30;
}

for (const p of ["obra", "rec-obra"]) $(`#${p}-tipo`).addEventListener("change", () => ajustarTipoObra(p));

function limparObra(p = "obra") {
  for (const el of $$(`#bloco-${p} input`)) el.value = "";
  $(`#${p}-cep-cidade`).textContent = "";
  $(`#${p}-tipo`).value = "cno";
  ajustarTipoObra(p);
}

/** Grupo `obra` do pedido (`nfse_core::modelo::Obra` em JSON), ou null se o serviço não pede. */
export function obraDigitada(p = "obra") {
  if ($(`#bloco-${p}`).hidden) return null;
  const valor = (campo) => $(`#${p}-${campo}`).value;
  const tipo = valor("tipo");
  const identificacao =
    tipo === "endereco"
      ? {
          endereco: {
            cep: soDigitos(valor("cep")),
            logradouro: valor("logradouro").trim(),
            numero: valor("numero").trim(),
            complemento: vazio(valor("complemento")),
            bairro: valor("bairro").trim(),
          },
        }
      : { [tipo]: valor("codigo").trim() };
  return { inscricao_imobiliaria: vazio(valor("inscricao")), identificacao };
}

export function preencherObra(obra, p = "obra") {
  limparObra(p);
  if (!obra) return;
  $(`#${p}-inscricao`).value = obra.inscricao_imobiliaria ?? "";
  const [tipo, dado] = Object.entries(obra.identificacao)[0];
  $(`#${p}-tipo`).value = tipo;
  ajustarTipoObra(p);
  if (tipo === "endereco") {
    $(`#${p}-cep`).value = FORMATOS.cep.mascara(dado.cep);
    $(`#${p}-logradouro`).value = dado.logradouro;
    $(`#${p}-numero`).value = dado.numero;
    $(`#${p}-complemento`).value = dado.complemento ?? "";
    $(`#${p}-bairro`).value = dado.bairro;
  } else {
    $(`#${p}-codigo`).value = dado;
  }
}

function descreverObra(obra) {
  const [tipo, dado] = Object.entries(obra.identificacao)[0];
  const id =
    tipo === "endereco"
      ? `${dado.logradouro}, ${dado.numero}${dado.complemento ? " " + dado.complemento : ""} — ${dado.bairro}, CEP ${FORMATOS.cep.mascara(dado.cep)}`
      : `${tipo === "cib" ? "CIB" : "CNO/CEI"} ${dado}`;
  return obra.inscricao_imobiliaria ? `${id} (inscr. imob. ${obra.inscricao_imobiliaria})` : id;
}

/** Volta o local da prestação ao município da empresa (campos em branco). */
function limparLocalPrestacao() {
  $("#local-cep").value = "";
  $("#local-prestacao").value = "";
  $("#local-cidade").textContent = "";
}

/** Código IBGE digitado, ou `null` se vazio ou igual ao município da empresa. */
function localPrestacao() {
  const codigo = soDigitos($("#local-prestacao").value);
  return codigo && codigo !== estado.empresa.municipio_emissor ? codigo : null;
}

export function limparEmissao() {
  encerrarSubstituicao();
  escolherTomador(null);
  $("#valor").value = "";
  $("#info-compl").value = "";
  limparLocalPrestacao();
  limparObra();
  $("#mes-ref").value = mesPadrao();
  $("#revisao").hidden = true;
  $("#resultado").hidden = true;
}

export function escolherTomador(t) {
  if (t !== estado.tomador) descartarRevisao();
  estado.tomador = t;
  mostrarTomadorEscolhido(
    {
      escolhido: $("#tomador-escolhido"),
      busca: $("#busca-tomador"),
      resultados: $("#resultados-tomador"),
      termo: $("#termo-tomador"),
      rotulo: $("#tomador-escolhido-texto"),
    },
    t && `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`,
  );
}

$("#trocar-tomador").addEventListener("click", () => {
  escolherTomador(null);
  $("#termo-tomador").focus();
});

ligarBuscaTomador($("#termo-tomador"), $("#resultados-tomador"), (t) => escolherTomador(t));

$("#novo-tomador-emitir").addEventListener("click", (ev) => {
  ev.preventDefault();
  editarTomador(null, (salvo) => escolherTomador(salvo));
});

/**
 * Nota autorizada que a emissão em curso substitui (`{ nfse_id, rotulo }`), ou `null`. O motivo e
 * a descrição ficam no bloco "Substituição de nota", no topo do formulário.
 */
let substituicao = null;

export function iniciarSubstituicao(nfseId, rotulo) {
  substituicao = { nfse_id: nfseId, rotulo };
  $("#subst-titulo").textContent = `Substituição da ${rotulo}`;
  $("#subst-motivo").value = "99";
  $("#subst-descricao").value = "";
  $("#bloco-substituicao").hidden = false;
}

function encerrarSubstituicao() {
  substituicao = null;
  $("#bloco-substituicao").hidden = true;
  $("#subst-descricao").value = "";
}

$("#desistir-substituicao").addEventListener("click", () => {
  encerrarSubstituicao();
  descartarRevisao();
  $("#resultado").hidden = true;
});

function montarPedido() {
  if (!estado.empresa) throw "Escolha a empresa.";
  const servicoId = Number($("#servico").value);
  if (!servicoId) throw new ErroCampo("servico", 'Escolha o serviço (cadastre um em "Serviços").');
  const valor = valorDigitado($("#valor").value);
  if (!valor) throw new ErroCampo("valor", "Informe o valor.");
  return {
    empresa_id: estado.empresa.id,
    tomador: estado.tomador?.pessoa_id ?? null,
    servico_id: servicoId,
    valor,
    descricao: vazio($("#descricao").value),
    mes_referencia: vazio($("#mes-ref").value),
    obra: obraDigitada(),
    local_prestacao: localPrestacao(),
    informacoes_complementares: vazio($("#info-compl").value),
    reter_federais: estado.reter?.federais ?? null,
    reter_iss: estado.reter?.iss ?? null,
    reter_irrf_ate_o_limite: estado.reter?.irrfAteOLimite ?? false,
    reter_contribuicoes_ate_o_limite: estado.reter?.contribuicoesAteOLimite ?? false,
    substituir: substituicao && {
      nfse_id: substituicao.nfse_id,
      motivo: $("#subst-motivo").value,
      descricao_motivo: vazio($("#subst-descricao").value),
    },
  };
}

/**
 * A revisão mostra o que será emitido (`estado.pedido`); se o formulário muda depois, ela deixa de
 * valer e fecha, para o "Emitir nota" (ou o Ctrl+Enter) não mandar o pedido antigo.
 */
export let emissaoEmCurso = false;

export function descartarRevisao() {
  // Durante o envio a revisão fica: o pedido já foi capturado e a resposta aparece nela.
  if (emissaoEmCurso) return;
  // revisão pedida e ainda sem resposta: a resposta, com os dados antigos, não abre mais
  if ($("#revisar").disabled) {
    revisaoAtual++;
    $("#revisar").disabled = false;
  }
  if ($("#revisao").hidden) return;
  $("#revisao").hidden = true;
  estado.pedido = null;
  anunciar("Os dados mudaram: revise a nota de novo antes de emitir.");
}

for (const evento of ["input", "change"]) {
  $('section[data-tela="emitir"] .cartao').addEventListener(evento, descartarRevisao);
}

$("#revisar").addEventListener("click", () => {
  estado.reter = null; // volta ao que o cadastro do tomador diz
  return revisar();
});

/** Cada revisão ganha um número; a resposta de uma revisão mais antiga que a última é ignorada. */
let revisaoAtual = 0;

async function revisar() {
  $("#resultado").hidden = true;
  const formulario = $('section[data-tela="emitir"] .cartao');
  limparErros(formulario);
  const erros = validarCampos(formulario);
  if (erros.length) return mostrarErros(erros);
  let pedido;
  try {
    pedido = montarPedido();
  } catch (e) {
    if (e instanceof ErroCampo) return mostrarErros([{ el: $(`#${e.campo}`), msg: e.message }]);
    return toast(String(e?.message ?? e), true);
  }
  const minha = ++revisaoAtual;
  const botao = $("#revisar");
  botao.disabled = true;
  let previa;
  try {
    previa = await chamar("previa_dps", { pedido });
  } catch (e) {
    if (minha === revisaoAtual) toast(String(e), true);
    return;
  } finally {
    if (minha === revisaoAtual) botao.disabled = false;
  }
  if (minha !== revisaoAtual) return;
  estado.pedido = pedido;
  const aviso = $("#revisao-aviso");
  const partes = [];
  if (previa.ambiente === "producao") {
    partes.push(`<div class="aviso erro"><strong>PRODUÇÃO:</strong> a nota terá valor fiscal.</div>`);
  } else {
    partes.push(`<div class="aviso atencao">Homologação: nota de teste, sem valor fiscal.</div>`);
  }
  if (previa.erros.length) {
    partes.push(`<div class="aviso erro"><strong>Corrija antes de emitir:</strong>
      <ul>${previa.erros.map((e) => `<li>${esc(e)}</li>`).join("")}</ul></div>`);
  }
  for (const texto of previa.avisos) partes.push(`<div class="aviso atencao">${esc(texto)}</div>`);
  aviso.innerHTML = partes.join("");
  $("#revisao-resumo").innerHTML = resumoValores(previa);
  const linhas = [];
  if (pedido.substituir) linhas.push(["Substitui", substituicao?.rotulo ?? ""]);
  linhas.push(
    ["Prestador", previa.prestador],
    ["Tomador", previa.tomador ?? "Não identificado"],
    ["Valor", "R$ " + moeda(previa.valor)],
    ["Competência", previa.competencia],
    ["Descrição", previa.descricao],
  );
  const tecnicos = [["Série da DPS", previa.serie]];
  if (pedido.obra) linhas.push(["Obra", descreverObra(pedido.obra)]);
  if (pedido.local_prestacao) {
    const cidade = $("#local-cidade").textContent;
    linhas.push(["Local da prestação", `${pedido.local_prestacao}${cidade ? ` (${cidade})` : ""}`]);
  }
  if (pedido.informacoes_complementares) linhas.push(["Inf. complementares", pedido.informacoes_complementares]);
  const reais = (v) => "R$ " + moeda(v);
  if (previa.pis_proprio) linhas.push(["PIS (apuração própria)", reais(previa.pis_proprio)]);
  if (previa.cofins_proprio) linhas.push(["COFINS (apuração própria)", reais(previa.cofins_proprio)]);
  if (previa.retencao_irrf) linhas.push(["IRRF retido", reais(previa.retencao_irrf)]);
  if (previa.retencao_cp) linhas.push(["INSS retido", reais(previa.retencao_cp)]);
  if (previa.retencao_csll) linhas.push(["CSLL/PIS/COFINS retidos", reais(previa.retencao_csll)]);
  if (previa.iss_retido) {
    linhas.push(["ISS", previa.iss_retido_valor ? "Retido pelo tomador: " + reais(previa.iss_retido_valor) : "Retido pelo tomador"]);
  }
  if (previa.valor_liquido) {
    const semIss = previa.iss_retido && !previa.iss_retido_valor;
    linhas.push(["Valor líquido", reais(previa.valor_liquido) + (semIss ? " (antes do ISS retido)" : "")]);
  }
  if (previa.ibs_cbs) {
    const i = previa.ibs_cbs;
    tecnicos.push(["IBS/CBS", `cIndOp ${i.indicador_operacao} · CST ${i.cst} · cClassTrib ${i.classificacao_tributaria}`]);
    tecnicos.push(["Base do IBS/CBS (estimada)", reais(i.base_estimada) + " (a Sefin calcula as alíquotas e os valores)"]);
  }
  const lista = (itens) => itens.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("");
  $("#revisao-dados").innerHTML = lista(linhas);
  $("#revisao-tecnico-dados").innerHTML = lista(tecnicos);
  $("#revisao-tecnico").open = false;
  const xml = $("#revisao-xml");
  xml.hidden = !previa.xml;
  xml.open = false;
  $("#revisao-xml-texto").textContent = previa.xml ?? "";
  const reter = $("#revisao-reter");
  reter.hidden = !previa.retencao_condicional;
  $("#reter-federais").checked = previa.reter_federais;
  $("#reter-iss").checked = previa.reter_iss;
  $("#revisao-limite").hidden = !previa.irrf_dispensavel && !previa.contribuicoes_dispensaveis;
  $("#linha-irrf-limite").hidden = !previa.irrf_dispensavel;
  $("#texto-irrf-limite").textContent = `Reter o IRRF de ${reais(previa.irrf_dispensavel ?? "0")}`;
  $("#reter-irrf-limite").checked = previa.reter_irrf_ate_o_limite;
  $("#linha-contribuicoes-limite").hidden = !previa.contribuicoes_dispensaveis;
  $("#texto-contribuicoes-limite").textContent =
    `Reter PIS/COFINS/CSLL de ${reais(previa.contribuicoes_dispensaveis ?? "0")}`;
  $("#reter-contribuicoes-limite").checked = previa.reter_contribuicoes_ate_o_limite;
  $("#confirmar-emissao").disabled = previa.erros.length > 0;
  $("#revisao").hidden = false;
  $("#revisao").scrollIntoView({ behavior: "smooth" });
}

/** "1500.00" → centavos, sem ponto flutuante. */
function centavos(valor) {
  const [inteiro, frac = ""] = String(valor ?? "0").split(".");
  return BigInt(inteiro || "0") * 100n + BigInt((frac + "00").slice(0, 2));
}

const reaisDeCentavos = (c) => "R$ " + moeda(`${c / 100n}.${String(c % 100n).padStart(2, "0")}`);

/** Valor da nota, o que o cliente desconta (retenções) e o que a empresa recebe, em linguagem simples. */
function resumoValores(previa) {
  const total = centavos(previa.valor);
  const liquido = previa.valor_liquido ? centavos(previa.valor_liquido) : total;
  const caixa = (rotulo, valor, classe = "") =>
    `<div class="${classe}"><span class="rotulo">${esc(rotulo)}</span><strong class="dado">${esc(valor)}</strong></div>`;
  const caixas = [caixa("Valor da nota", reaisDeCentavos(total))];
  if (total !== liquido) caixas.push(caixa("O cliente desconta (retenções)", "− " + reaisDeCentavos(total - liquido)));
  caixas.push(caixa("A empresa recebe", reaisDeCentavos(liquido), "destaque"));
  const notas = [];
  if (total === liquido && !previa.iss_retido) notas.push("Sem retenções: o cliente paga o valor cheio da nota.");
  if (previa.iss_retido && !previa.iss_retido_valor) {
    notas.push(
      "O cliente também desconta o ISS retido, calculado pela Sefin; para ver o valor aqui, informe a alíquota do ISS no cadastro do serviço.",
    );
  }
  const texto = notas.map((n) => `<p class="ajuda resumo-nota">${esc(n)}</p>`).join("");
  return `<div class="resumo-valores">${caixas.join("")}</div>${texto}`;
}

/** "Reter nesta nota" e "Retenção de até R$ 10,00": refaz a prévia com a escolha, que vale mais que o cadastro do tomador e a dispensa. */
for (const id of ["#reter-federais", "#reter-iss", "#reter-irrf-limite", "#reter-contribuicoes-limite"]) {
  $(id).addEventListener("change", () => {
    estado.reter = {
      federais: $("#reter-federais").checked,
      iss: $("#reter-iss").checked,
      irrfAteOLimite: $("#reter-irrf-limite").checked,
      contribuicoesAteOLimite: $("#reter-contribuicoes-limite").checked,
    };
    revisar();
  });
}

$("#voltar-edicao").addEventListener("click", () => {
  $("#revisao").hidden = true;
  $("#valor").focus();
});

$("#confirmar-emissao").addEventListener("click", async () => {
  // O pedido revisado; mudar o formulário durante o envio não troca o que já foi enviado.
  const pedido = estado.pedido;
  if (!pedido) return;
  if (atualizando) return toast("A atualização do sistema está em andamento; emita depois que ele abrir de novo.", true);
  emissaoEmCurso = true;
  const botao = $("#confirmar-emissao");
  botao.disabled = true;
  botao.textContent = "Enviando à Sefin...";
  const res = $("#resultado");
  try {
    const r = await chamar("emitir", { pedido });
    $("#revisao").hidden = true;
    if (r.autorizada) {
      res.innerHTML = `<div class="aviso ok"><div><strong>NFS-e ${esc(r.numero ?? "")} autorizada.</strong><br>
          Chave: <code>${esc(r.chave)}</code></div></div>
        ${r.avisos
          .map((a) => `<div class="aviso ${a.startsWith("E-mail enviado") ? "ok" : "atencao"}">${esc(a)}</div>`)
          .join("")}
        <div class="acoes inicio">
          <button class="botao primario" data-abrir="pdf">Abrir PDF</button>
          <button class="botao" data-abrir="pasta">Abrir pasta</button>
          ${r.avisos.some((a) => a.startsWith("E-mail enviado")) ? "" : `<button class="botao" data-email>Enviar por e-mail</button>`}
          ${estado.tomador ? `<button class="botao" data-outra title="Mantém o tomador e o serviço; só falta o valor">Emitir outra para este tomador</button>` : ""}
          <button class="botao" data-recorrente title="Cria uma recorrência com este tomador, serviço, valor e descrição">Tornar recorrente</button>
        </div>`;
      res.querySelector('[data-abrir="pdf"]').onclick = () => abrirNota(r.nfse_id, false);
      res.querySelector('[data-abrir="pasta"]').onclick = () => abrirNota(r.nfse_id, true);
      const emitido = pedido;
      const tomadorEmitido = estado.tomador;
      const nota = {
        nfse_id: r.nfse_id,
        numero_nfse: r.numero,
        tomador: tomadorEmitido?.tomador.nome ?? null,
        situacao: "autorizada",
      };
      const botaoEmail = res.querySelector("[data-email]");
      if (botaoEmail) botaoEmail.onclick = () => enviarEmailNota(nota);
      const botaoOutra = res.querySelector("[data-outra]");
      if (botaoOutra) {
        botaoOutra.onclick = () => {
          escolherTomador(tomadorEmitido);
          $("#servico").value = String(emitido.servico_id);
          aplicarServico();
          res.hidden = true;
          $("#valor").select();
          $("#valor").focus();
        };
      }
      res.querySelector("[data-recorrente]").onclick = () =>
        editarRecorrencia(null, {
          tomador_id: tomadorEmitido?.pessoa_id,
          tomador: tomadorEmitido
            ? `${tomadorEmitido.tomador.nome} — ${formatarDocumento(tomadorEmitido.tomador.documento)}`
            : null,
          servico_id: emitido.servico_id,
          valor: emitido.valor.replace(",", "."),
          descricao: emitido.descricao,
          informacoes_complementares: emitido.informacoes_complementares,
          obra: emitido.obra,
        });
      // Prepara a próxima nota (mesmo serviço e mês) sem permitir reenviar a anterior por engano.
      encerrarSubstituicao();
      escolherTomador(null);
      $("#valor").value = "";
      $("#info-compl").value = "";
      limparLocalPrestacao();
      limparObra();
      aplicarServico();
      toast(`NFS-e ${r.numero ?? ""} autorizada.`);
    } else {
      mostrarIncerta(res, r);
    }
  } catch (e) {
    res.innerHTML = `<div class="aviso erro"><div><strong>Nota não emitida.</strong><br>${esc(e)}</div></div>`;
    oferecerCorrecoes(res, String(e), pedido);
  } finally {
    emissaoEmCurso = false;
    res.hidden = false;
    botao.disabled = false;
    botao.textContent = "Emitir nota";
  }
});

/** Sem resposta definitiva: a nota pode ter saído; "Verificar agora" consulta a Sefin pela DPS. */
function mostrarIncerta(res, r) {
  res.innerHTML = `<div class="aviso atencao"><span><strong>Sem resposta definitiva da Sefin.</strong><br>${esc(r.motivo)}
      <br>Não emita esta nota de novo antes de verificar.</span></div>
    <div class="acoes inicio">
      <button class="botao primario" data-verificar>Verificar agora</button>
      <button class="botao" data-ir="notas">Ver em Notas emitidas</button>
    </div>`;
  const botao = res.querySelector("[data-verificar]");
  botao.onclick = async () => {
    botao.disabled = true;
    try {
      const msg = await chamar("reconciliar", { dpsId: r.dps_id });
      res.innerHTML = `<div class="aviso ok">${esc(msg)}</div>
        <div class="acoes inicio"><button class="botao" data-ir="notas">Ver em Notas emitidas</button></div>`;
    } catch (e) {
      // Nos primeiros minutos a Sefin pode ainda não mostrar a nota: a mensagem diz quando tentar de novo.
      toast(String(e), true);
      botao.disabled = false;
    }
  };
}

/**
 * Botões que resolvem as rejeições com orientação conhecida (`nfse-nacional/src/mensagens.rs`);
 * a orientação em texto já vem no erro. Cada código leva à tela ou ao diálogo da correção.
 */
const CORRECOES = {
  E0014: [{ texto: "Ajustar a numeração pela Sefin", fazer: ajustarNumeracaoAposRejeicao }],
  E0084: [{ texto: "Editar a empresa", fazer: (p) => editarEmpresa(empresaDoPedido(p)) }],
  E0116: [
    { texto: "Editar a empresa", fazer: (p) => editarEmpresa(empresaDoPedido(p)) },
    { texto: "Abrir Numeração / IM", fazer: (p) => editarNumeracao(empresaDoPedido(p)) },
  ],
  E0120: [{ texto: "Abrir Numeração / IM", fazer: (p) => editarNumeracao(empresaDoPedido(p)) }],
  E0312: [{ texto: "Editar o serviço", fazer: (p) => editarServicoDoPedido(p) }],
  E0710: [{ texto: "Editar o serviço", fazer: (p) => editarServicoDoPedido(p) }],
  E0712: [{ texto: "Editar o serviço", fazer: (p) => editarServicoDoPedido(p) }],
  E0713: [{ texto: "Editar o serviço", fazer: (p) => editarServicoDoPedido(p) }],
  E0714: [{ texto: "Importar o certificado de novo", fazer: (p) => importarCertificado(empresaDoPedido(p)) }],
  E0717: [{ texto: "Importar o certificado de novo", fazer: (p) => importarCertificado(empresaDoPedido(p)) }],
};

const empresaDoPedido = (pedido) => estado.empresas.find((e) => e.id === pedido.empresa_id);

function editarServicoDoPedido(pedido) {
  const s = estado.servicos.find((x) => x.id === pedido.servico_id);
  if (!s) return toast('O serviço não está mais no cadastro; confira em "Serviços".', true);
  editarServico(s);
}

function oferecerCorrecoes(res, erro, pedido) {
  const codigos = [...new Set(erro.match(/\bE\d{4}\b/g) ?? [])];
  const correcoes = codigos.flatMap((c) => CORRECOES[c] ?? []).filter(
    (c, i, todas) => todas.findIndex((x) => x.texto === c.texto) === i,
  );
  if (!correcoes.length || !empresaDoPedido(pedido)) return;
  const acoes = document.createElement("div");
  acoes.className = "acoes inicio";
  for (const c of correcoes) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "botao";
    b.textContent = c.texto;
    b.onclick = () => c.fazer(pedido, b);
    acoes.append(b);
  }
  res.append(acoes);
}

/** E0014: a série já foi usada; consulta a Sefin, ajusta o próximo número e deixa a revisão pronta para reenviar. */
async function ajustarNumeracaoAposRejeicao(pedido, botao) {
  botao.disabled = true;
  try {
    const msg = await chamar("ajustar_numeracao", { empresaId: pedido.empresa_id });
    const p = document.createElement("p");
    p.className = "ajuda";
    p.textContent = `${msg} Confira a revisão acima e clique em "Emitir nota" para enviar de novo.`;
    botao.replaceWith(p);
    $("#confirmar-emissao").focus();
  } catch (e) {
    toast(String(e), true);
    botao.disabled = false;
  }
}

export async function abrirNota(nfseId, pasta) {
  try {
    await invoke("abrir_nota", { nfseId, pasta });
  } catch (e) {
    toast(String(e), true);
  }
}
