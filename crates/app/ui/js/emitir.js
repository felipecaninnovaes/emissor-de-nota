// Tela de emissão: tomador, serviço, obra, revisão e envio.

import {
  $,
  $$,
  chamar,
  esc,
  estado,
  formatarDocumento,
  invoke,
  mesAnterior,
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

export function renderizarServicosEmitir() {
  const sel = $("#servico");
  sel.innerHTML = estado.servicos.length
    ? estado.servicos.map((s) => `<option value="${s.id}">${esc(s.nome)}</option>`).join("")
    : `<option value="">Cadastre um serviço em "Serviços"</option>`;
  aplicarServico();
}

export function aplicarServico() {
  const s = estado.servicos.find((x) => x.id === Number($("#servico").value));
  if (!s) return;
  $("#descricao").value = s.descricao;
  if (s.valor_sugerido) $("#valor").value = moeda(s.valor_sugerido);
  $("#bloco-obra").hidden = !CODIGOS_COM_OBRA.includes(s.codigo_trib_nac);
}

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

export function limparEmissao() {
  escolherTomador(null);
  $("#valor").value = "";
  $("#info-compl").value = "";
  limparObra();
  $("#mes-ref").value = mesAnterior();
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
    informacoes_complementares: vazio($("#info-compl").value),
    reter_federais: estado.reter?.federais ?? null,
    reter_iss: estado.reter?.iss ?? null,
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
  const linhas = [
    ["Prestador", previa.prestador],
    ["Tomador", previa.tomador ?? "Não identificado"],
    ["Valor", "R$ " + moeda(previa.valor)],
    ["Competência", previa.competencia],
    ["Série da DPS", previa.serie],
    ["Descrição", previa.descricao],
  ];
  if (pedido.obra) linhas.push(["Obra", descreverObra(pedido.obra)]);
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
    linhas.push(["IBS/CBS", `cIndOp ${i.indicador_operacao} · CST ${i.cst} · cClassTrib ${i.classificacao_tributaria}`]);
    linhas.push(["Base do IBS/CBS (estimada)", reais(i.base_estimada) + " (a Sefin calcula as alíquotas e os valores)"]);
  }
  $("#revisao-dados").innerHTML = linhas.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("");
  const xml = $("#revisao-xml");
  xml.hidden = !previa.xml;
  xml.open = false;
  $("#revisao-xml-texto").textContent = previa.xml ?? "";
  const reter = $("#revisao-reter");
  reter.hidden = !previa.retencao_condicional;
  $("#reter-federais").checked = previa.reter_federais;
  $("#reter-iss").checked = previa.reter_iss;
  $("#confirmar-emissao").disabled = previa.erros.length > 0;
  $("#revisao").hidden = false;
  $("#revisao").scrollIntoView({ behavior: "smooth" });
}

/** "Reter nesta nota": refaz a prévia com a escolha, que vale mais que o cadastro do tomador. */
for (const id of ["#reter-federais", "#reter-iss"]) {
  $(id).addEventListener("change", () => {
    estado.reter = { federais: $("#reter-federais").checked, iss: $("#reter-iss").checked };
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
      res.innerHTML = `<div class="aviso ok"><strong>NFS-e ${esc(r.numero ?? "")} autorizada.</strong><br>
          Chave: <code>${esc(r.chave)}</code></div>
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
      escolherTomador(null);
      $("#info-compl").value = "";
      limparObra();
      aplicarServico();
      toast(`NFS-e ${r.numero ?? ""} autorizada.`);
    } else {
      res.innerHTML = `<div class="aviso atencao"><strong>Sem resposta definitiva da Sefin.</strong><br>${esc(r.motivo)}
        <br>Confira em "Notas emitidas" (botão Verificar) antes de emitir de novo.</div>`;
    }
  } catch (e) {
    res.innerHTML = `<div class="aviso erro"><strong>Nota não emitida.</strong><br>${esc(e)}</div>`;
  } finally {
    emissaoEmCurso = false;
    res.hidden = false;
    botao.disabled = false;
    botao.textContent = "Emitir nota";
  }
});

export async function abrirNota(nfseId, pasta) {
  try {
    await invoke("abrir_nota", { nfseId, pasta });
  } catch (e) {
    toast(String(e), true);
  }
}
