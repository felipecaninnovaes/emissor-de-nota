// Notas emitidas: lista, e-mail, cancelamento, refazer, copiar e substituir.

import { $, NOME_SITUACAO, chamar, copiarTexto, esc, estado, invoke, moeda, somarValores, toast, vazio } from "./util.js";
import { acoesLinha, detalheLinha, linhaVazia, selecaoEmTabela } from "./tabela.js";
import { ErroCampo, abrirDialogo, atualizarContador } from "./formularios.js";
import { descreverMunicipio } from "./componentes.js";
import { irPara } from "./navegacao.js";
import { abrirNota, aplicarServico, escolherTomador, iniciarSubstituicao, limparEmissao, preencherObra } from "./emitir.js";
import { atualizando } from "./atualizacao.js";

let notasCarregadas = [];

// Só a nota autorizada (com NFS-e) entra no cancelamento em lote.
const podeCancelar = (n) => n.situacao === "autorizada" && !!n.nfse_id;
const selecaoNotas = selecaoEmTabela({
  lista: "#lista-notas",
  itens: () => notasCarregadas,
  botao: "#notas-cancelar-lote",
  rotulo: "Cancelar selecionadas",
  todos: "#notas-todas",
  emAndamento: () => cancelamentoEmCurso,
});
/** O botão do lote só aparece com alguma nota marcada. */
function atualizarSelecao() {
  selecaoNotas.atualizar();
  $("#notas-cancelar-lote").hidden = !selecaoNotas.marcados().length;
}
$("#lista-notas").addEventListener("change", atualizarSelecao);
$("#notas-todas").addEventListener("change", atualizarSelecao);

export async function carregarNotas() {
  const corpo = $("#lista-notas");
  $("#rodape-notas").innerHTML = "";
  // recarregar (troca de mês ou de empresa) limpa as marcações
  $("#notas-todas").checked = false;
  try {
    await desenharNotas(corpo);
  } finally {
    atualizarSelecao();
  }
}

async function desenharNotas(corpo) {
  if (!estado.empresa) {
    notasCarregadas = [];
    corpo.innerHTML = linhaVazia(9, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  notasCarregadas = await invoke("listar_notas", { empresaId: estado.empresa.id, mes: vazio($("#mes-notas").value) });
  if (!notasCarregadas.length) {
    corpo.innerHTML = linhaVazia(9, "Nenhuma nota neste período.", { ir: "emitir", texto: "Emitir nota" });
    return;
  }
  corpo.innerHTML = notasCarregadas
    .map((n, i) => {
      // a ação mais usada em cada situação fica visível; as outras vão para "Mais"
      const acoes = [];
      if (n.situacao === "rejeitada") acoes.push({ acao: "refazer", texto: "Corrigir e emitir" });
      if (n.situacao === "incerta" || n.situacao === "enviando") acoes.push({ acao: "verificar", texto: "Verificar" });
      if (["rejeitada", "incerta", "enviando"].includes(n.situacao)) {
        acoes.push({ acao: "diagnostico", texto: "Diagnóstico", menu: true });
      }
      if (n.nfse_id) {
        acoes.push({ acao: "pdf", texto: "PDF" }, { acao: "pasta", texto: "Pasta" });
        if (n.situacao === "autorizada" || n.situacao === "cancelada") {
          acoes.push({ acao: "email", texto: "E-mail" }, { acao: "copiar", texto: "Copiar" });
        }
        if (n.situacao === "autorizada") {
          acoes.push({ acao: "conferir", texto: "Conferir na Sefin" });
          acoes.push({ acao: "substituir", texto: "Substituir nota" });
          acoes.push({ acao: "cancelar", texto: "Cancelar nota", perigo: true });
        }
      }
      const alvo = n.numero_nfse ? `nota ${n.numero_nfse}` : `DPS ${n.serie}/${n.numero_dps}`;
      const motivo = n.erro && n.situacao !== "autorizada" ? esc(n.erro) : "";
      const det = detalheLinha({
        id: `det-nota-${i}`,
        colunas: 9,
        alvo,
        itens: [
          ["Tomador", esc(n.tomador ?? "Não identificado")],
          ["Descrição", esc(n.descricao ?? "")],
          ["DPS", `<span class="dado">${esc(n.serie)}/${n.numero_dps}</span>`],
          ["E-mail", n.email_enviado_em ? `enviado em ${esc(dataHora(n.email_enviado_em))}` : ""],
        ],
        motivo,
        erro: n.situacao === "rejeitada",
      });
      const email = n.email_enviado_em ? `E-mail enviado em ${dataHora(n.email_enviado_em)}` : "";
      return `<tr>
        <td><input type="checkbox" data-i="${i}" ${podeCancelar(n) ? "" : "disabled"} aria-label="${esc(`Marcar a ${alvo} para cancelar`)}"></td>
        <td>${det.botao}</td>
        <td class="dado">${esc(n.numero_nfse ?? "—")}</td>
        <td class="dado sec-2">${esc(n.serie)}/${n.numero_dps}</td>
        <td title="${esc(n.tomador)}">${esc(n.tomador ?? "—")}</td>
        <td class="sec-1" title="${esc(n.descricao)}">${esc(n.descricao ?? "")}</td>
        <td class="num">${n.valor ? "R$ " + moeda(n.valor) : ""}</td>
        <td><span class="selo ${esc(n.situacao)}" title="${esc(email)}">${esc(NOME_SITUACAO[n.situacao] ?? n.situacao)}</span></td>
        <td>${acoesLinha(acoes, i, alvo)}</td>
      </tr>${det.linha}`;
    })
    .join("");
  const autorizadas = notasCarregadas.filter((n) => n.situacao === "autorizada");
  const total = somarValores(autorizadas, (n) => n.valor);
  const outras = notasCarregadas.length - autorizadas.length;
  $("#rodape-notas").innerHTML = `<tr><td colspan="9">${notasCarregadas.length} nota(s) · ${autorizadas.length}
    autorizada(s), total <span class="dado">R$ ${moeda(total)}</span>${
      outras ? ` · ${outras} em outra situação` : ""
    }</td></tr>`;
}

$("#mes-notas").addEventListener("change", () => carregarNotas().catch((e) => toast(String(e), true)));
$("#todas-notas").addEventListener("click", () => {
  $("#mes-notas").value = "";
  carregarNotas().catch((e) => toast(String(e), true));
});

// Notas canceladas fora do sistema (portal nacional) só aparecem aqui depois desta conferência.
$("#conferir-cancelamentos").addEventListener("click", async () => {
  const botao = $("#conferir-cancelamentos");
  const autorizadas = notasCarregadas.filter((n) => n.situacao === "autorizada" && n.nfse_id);
  if (!autorizadas.length) return toast("Nenhuma nota autorizada na lista para conferir.");
  botao.disabled = true;
  let canceladas = 0;
  let falhas = 0;
  let primeiraFalha = "";
  try {
    for (const [i, n] of autorizadas.entries()) {
      botao.textContent = `Conferindo ${i + 1} de ${autorizadas.length}...`;
      try {
        if (await chamar("sincronizar_cancelamento", { nfseId: n.nfse_id })) canceladas++;
      } catch (e) {
        falhas++;
        primeiraFalha ||= `nota ${n.numero_nfse ?? ""}: ${e}`;
      }
    }
  } finally {
    botao.disabled = false;
    botao.textContent = "Conferir cancelamentos";
  }
  const resumo =
    `${autorizadas.length} nota(s) conferida(s); ` +
    (canceladas ? `${canceladas} cancelada(s) na Sefin, agora registrada(s).` : "nenhuma cancelada fora do sistema.");
  toast(falhas ? `${resumo} ${falhas} sem resposta (${primeiraFalha}).` : resumo, falhas > 0);
  if (canceladas) await carregarNotas().catch((e) => toast(String(e), true));
});

$("#abrir-pasta-mes").addEventListener("click", async () => {
  const comArquivo = notasCarregadas.find((n) => n.nfse_id);
  if (comArquivo) return abrirNota(comArquivo.nfse_id, true);
  try {
    await invoke("abrir_arquivo", { caminho: await invoke("obter_pasta") });
  } catch (e) {
    toast(String(e), true);
  }
});

$("#lista-notas").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const n = notasCarregadas[Number(b.dataset.i)];
  switch (b.dataset.acao) {
    case "pdf":
      return abrirNota(n.nfse_id, false);
    case "pasta":
      return abrirNota(n.nfse_id, true);
    case "cancelar":
      return cancelarNota(n);
    case "email":
      return enviarEmailNota(n);
    case "refazer":
      return refazerNota(n);
    case "copiar":
      return copiarNota(n);
    case "substituir":
      return substituirNota(n);
    case "diagnostico":
      try {
        await copiarTexto(await chamar("diagnostico_nota", { dpsId: n.dps_id }));
        toast("Diagnóstico copiado (dados pessoais omitidos).");
      } catch (e) {
        toast(String(e), true);
      }
      return;
    case "conferir":
      try {
        const cancelada = await chamar("sincronizar_cancelamento", { nfseId: n.nfse_id });
        toast(cancelada ? "A nota foi cancelada na Sefin; cancelamento registrado." : "A nota continua autorizada na Sefin.");
        if (cancelada) await carregarNotas();
      } catch (e) {
        toast(String(e), true);
      }
      return;
    case "verificar":
      b.disabled = true;
      try {
        toast(await chamar("reconciliar", { dpsId: n.dps_id }));
        await carregarNotas();
      } catch (e) {
        toast(String(e), true);
        b.disabled = false;
      }
  }
});

/**
 * Preenche a tela de emissão com os dados de uma nota (`comando`: dados_para_refazer ou
 * dados_para_copiar). Com `manterDescricao`, fica o texto enviado; senão, o do modelo do serviço,
 * a não ser que o serviço não exista mais no cadastro.
 */
async function levarParaEmissao(comando, n, { manterDescricao, aviso, orientacao }) {
  let r;
  try {
    r = await chamar(comando, { dpsId: n.dps_id });
  } catch (e) {
    return toast(String(e), true);
  }
  irPara("emitir");
  limparEmissao();
  escolherTomador(r.tomador);
  const servicoAchado = r.servico_id && estado.servicos.some((s) => s.id === r.servico_id);
  if (servicoAchado) $("#servico").value = String(r.servico_id);
  aplicarServico(); // descrição do modelo do serviço e dados da obra
  $("#valor").value = moeda(r.valor);
  if (manterDescricao || !servicoAchado) $("#descricao").value = r.descricao;
  atualizarContador($("#descricao"));
  $("#info-compl").value = r.informacoes_complementares ?? "";
  $("#local-prestacao").value = r.local_prestacao ?? "";
  descreverMunicipio($("#local-prestacao"));
  preencherObra(r.obra);
  const avisos = [aviso];
  if (!servicoAchado) avisos.push("Nenhum serviço do cadastro tem o mesmo código: escolha o serviço.");
  if (n.tomador && !r.tomador) avisos.push(`O tomador "${n.tomador}" não foi encontrado no cadastro desta empresa.`);
  const res = $("#resultado");
  res.innerHTML = `<div class="aviso atencao">${avisos.map(esc).join("<br>")}
    <br>${orientacao} A nota sai com o próximo número da série.</div>`;
  res.hidden = false;
  return true;
}

/** Leva os dados de uma nota rejeitada para a tela de emissão (sai com o próximo número). */
function refazerNota(n) {
  return levarParaEmissao("dados_para_refazer", n, {
    manterDescricao: true, // já com o mês de referência aplicado
    aviso: `Refazendo a DPS ${n.serie}/${n.numero_dps}, rejeitada: ${n.erro ?? ""}`,
    orientacao: "Corrija o que causou a rejeição e clique em Revisar.",
  });
}

/**
 * Leva os dados de uma nota autorizada ou cancelada para a tela de emissão. A descrição vem do
 * modelo do serviço (com o mês de referência da tela): copiar o texto antigo repetiria o mês velho.
 */
function copiarNota(n) {
  return levarParaEmissao("dados_para_copiar", n, {
    manterDescricao: false,
    aviso: `Copiando a nota ${n.numero_nfse ?? `DPS ${n.serie}/${n.numero_dps}`}.`,
    orientacao: "Confira o mês de referência e o valor e clique em Revisar.",
  });
}

/**
 * Leva os dados de uma nota autorizada para a tela de emissão, como substituta dela. A descrição é
 * a enviada (o mês de referência é o da nota original); o motivo fica no topo do formulário.
 */
async function substituirNota(n) {
  const rotulo = `nota ${n.numero_nfse ?? `DPS ${n.serie}/${n.numero_dps}`}`;
  const levou = await levarParaEmissao("dados_para_substituir", n, {
    manterDescricao: true,
    aviso: `Substituindo a ${rotulo}.`,
    orientacao: "Corrija o que precisa mudar, escolha o motivo e clique em Revisar.",
  });
  if (levou) iniciarSubstituicao(n.nfse_id, rotulo);
}

/** "2026-10-16T22:07:26Z" → "16/10/2026 19:07" (horário local). */
function dataHora(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const dois = (x) => String(x).padStart(2, "0");
  return `${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()} ${dois(d.getHours())}:${dois(d.getMinutes())}`;
}

export async function enviarEmailNota(n) {
  let para = [];
  try {
    para = await chamar("destinatarios_email", { nfseId: n.nfse_id });
  } catch (e) {
    return toast(String(e), true);
  }
  const dlg = $("#dlg-email");
  const form = $("form", dlg);
  form.reset();
  $("#email-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"}${
    n.situacao === "cancelada" ? " (CANCELADA: vai o e-mail de cancelamento, com o motivo)" : ""
  }`;
  form.elements.para.value = para.join("; ");
  abrirDialogo(dlg, $("#erro-email"), async (d) => {
    toast(await chamar("enviar_email", { nfseId: n.nfse_id, para: d.para }));
    return carregarNotas;
  });
}

// Um lote (cancelamento ou e-mail) ou um cancelamento em curso: a atualização do sistema espera.
export let cancelamentoEmCurso = false;

const ATUALIZANDO = "A atualização do sistema está em andamento; cancele depois que ele abrir de novo.";

/** "Avisar o tomador por e-mail": marcada por padrão; sem e-mail configurado, desabilitada com a dica. */
async function prepararAviso(form) {
  let configurado = false;
  try {
    configurado = (await invoke("obter_email")).configurado;
  } catch {
    // sem a configuração legível, trata como não configurado: a caixa fica desabilitada com a dica
  }
  const caixa = $("[data-avisar]", form);
  caixa.checked = configurado;
  caixa.disabled = !configurado;
  $("[data-sem-email]", form).hidden = configurado;
}

/** Confere a justificativa no backend (mesmas regras do pedido à Sefin); o erro vai para o campo. */
async function conferirJustificativa(justificativa) {
  try {
    await chamar("conferir_justificativa_cancelamento", { justificativa });
  } catch (e) {
    throw new ErroCampo("justificativa", String(e));
  }
}

/** O que dizer sobre o e-mail de um cancelamento (vazio se o aviso não foi pedido). */
function textoEmail(email) {
  if (!email) return "";
  return email.enviado
    ? `E-mail enviado para ${email.para.join(", ")}.`
    : `E-mail não enviado: ${email.erro}. Reenvie pela ação "E-mail".`;
}

async function cancelarNota(n) {
  const dlg = $("#dlg-cancelar");
  const form = $("form", dlg);
  $("#cancelar-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"} — R$ ${moeda(n.valor)}`;
  form.reset();
  await prepararAviso(form);
  abrirDialogo(dlg, $("#erro-cancelar"), async (d) => {
    const justificativa = d.justificativa.trim();
    await conferirJustificativa(justificativa);
    if (atualizando) throw ATUALIZANDO;
    cancelamentoEmCurso = true;
    let r;
    try {
      r = await chamar("cancelar", { nfseId: n.nfse_id, motivo: d.motivo, justificativa, avisar: d.avisar });
    } finally {
      cancelamentoEmCurso = false;
    }
    if (r.situacao !== "cancelada") throw r.mensagem;
    const email = textoEmail(r.email);
    toast(`NFS-e ${n.numero_nfse ?? ""} cancelada.${email ? ` ${email}` : ""}`, !!r.email && !r.email.enviado);
    return carregarNotas;
  });
}

$("#notas-cancelar-lote").addEventListener("click", () =>
  cancelarSelecionadas(selecaoNotas.marcados()).catch((e) => toast(String(e), true)),
);

/** Diálogo do lote: lista, total, um motivo e uma justificativa para todas. */
async function cancelarSelecionadas(notas) {
  if (!notas.length) return;
  const dlg = $("#dlg-cancelar-lote");
  const form = $("form", dlg);
  form.reset();
  $("#lista-cancelar-lote").innerHTML = notas
    .map(
      (n) => `<tr><td class="dado">${esc(n.numero_nfse ?? "—")}</td><td title="${esc(n.tomador)}">${esc(n.tomador ?? "—")}</td>
        <td class="num">${n.valor ? "R$ " + moeda(n.valor) : ""}</td></tr>`,
    )
    .join("");
  const total = somarValores(notas, (n) => n.valor);
  $("#rodape-cancelar-lote").innerHTML = `<tr><td colspan="3">${notas.length} nota(s), total
    <span class="dado">R$ ${moeda(total)}</span></td></tr>`;
  const aviso = $("#aviso-cancelar-lote");
  aviso.hidden = notas.length <= 20;
  aviso.textContent = `São ${notas.length} notas. Confira a lista: o cancelamento não pode ser desfeito.`;
  $("#cancelar-lote-ok").textContent = notas.length === 1 ? "Cancelar 1 nota" : `Cancelar ${notas.length} notas`;
  $("#cancelar-lote-andamento").hidden = true;
  await prepararAviso(form);
  abrirDialogo(dlg, $("#erro-cancelar-lote"), async (d) => {
    const justificativa = d.justificativa.trim();
    // conferida uma vez, antes do primeiro envio: com erro, nenhuma nota vai à Sefin
    await conferirJustificativa(justificativa);
    if (atualizando) throw ATUALIZANDO;
    const resultados = await cancelarEmSequencia(notas, { motivo: d.motivo, justificativa, avisar: d.avisar }, form);
    return () => mostrarResumo(notas, resultados);
  });
}

/**
 * Roda `uma(item)` para cada item, um de cada vez (sem paralelismo), com o andamento no diálogo
 * ("Cancelando 2 de 5…"). `uma` devolve o resultado do item, inclusive o de falha, e a falha de um
 * não interrompe os outros. O diálogo não fecha no meio do lote.
 */
async function emSequencia(itens, { form, andamento, botao, verbo }, uma) {
  const dlg = form.closest("dialog");
  // o botão sempre visível também mostra o andamento (o parágrafo pode estar fora da rolagem)
  const textoBotao = botao.textContent;
  const oncancel = dlg.oncancel;
  dlg.oncancel = (ev) => ev.preventDefault();
  const travados = [...form.elements].filter((el) => !el.disabled);
  for (const el of travados) el.disabled = true;
  andamento.hidden = false;
  cancelamentoEmCurso = true;
  atualizarSelecao();
  const resultados = [];
  try {
    for (const [i, item] of itens.entries()) {
      andamento.textContent = `${verbo} ${i + 1} de ${itens.length}… Não feche o programa.`;
      botao.textContent = `${verbo} ${i + 1} de ${itens.length}…`;
      resultados.push(await uma(item));
    }
  } finally {
    cancelamentoEmCurso = false;
    for (const el of travados) el.disabled = false;
    dlg.oncancel = oncancel;
    andamento.hidden = true;
    botao.textContent = textoBotao;
    atualizarSelecao();
  }
  return resultados;
}

/** Cancela as notas em sequência; a falha de uma vira o resultado dela. */
function cancelarEmSequencia(notas, pedido, form) {
  const elementos = {
    form,
    andamento: $("#cancelar-lote-andamento"),
    botao: $("#cancelar-lote-ok"),
    verbo: "Cancelando",
  };
  return emSequencia(notas, elementos, (n) =>
    chamar("cancelar", { nfseId: n.nfse_id, ...pedido }).catch((e) => ({
      situacao: "recusada",
      mensagem: String(e),
      email: null,
    })),
  );
}

const rotuloNota = (n) => esc(`NFS-e ${n.numero_nfse ?? "—"} — ${n.tomador ?? "sem tomador"}`);

/** Um grupo do resumo de um lote (nada, se não tiver itens). */
const blocoResumo = (classe, titulo, itens, nota = "") =>
  itens.length ? `<div class="aviso ${classe}"><div><b>${titulo}</b>${nota}<ul>${itens.join("")}</ul></div></div>` : "";

/** Resumo de um lote (cancelamento ou e-mail); fechar recarrega a lista. */
function abrirResumo(titulo, html) {
  $("#titulo-dlg-resumo-lote").textContent = titulo;
  $("#resumo-lote").innerHTML = html;
  const dlg = $("#dlg-resumo-lote");
  dlg.addEventListener("close", () => carregarNotas().catch((e) => toast(String(e), true)), { once: true });
  dlg.showModal();
}

/** Resumo do cancelamento em lote, em grupos. */
function mostrarResumo(notas, resultados) {
  const grupos = { cancelada: [], recusada: [], sem_confirmacao: [] };
  resultados.forEach((r, i) => (grupos[r.situacao] ?? grupos.recusada).push([notas[i], r]));
  const semEmail = grupos.cancelada.filter(([, r]) => r.email && !r.email.enviado);
  abrirResumo(
    "Resultado do cancelamento",
    blocoResumo(
      "ok",
      `${grupos.cancelada.length} cancelada(s)`,
      grupos.cancelada.map(([n, r]) => `<li>${rotuloNota(n)}${r.email?.enviado ? " · e-mail enviado" : ""}</li>`),
    ) +
      blocoResumo(
        "atencao",
        `${semEmail.length} cancelada(s), e-mail não enviado`,
        semEmail.map(([n, r]) => `<li>${rotuloNota(n)}: ${esc(r.email.erro)}</li>`),
        `<p>Reenvie pela ação "E-mail" da nota.</p>`,
      ) +
      blocoResumo(
        "erro",
        `${grupos.recusada.length} não cancelada(s)`,
        grupos.recusada.map(([n, r]) => `<li>${rotuloNota(n)}: ${esc(r.mensagem ?? "")}</li>`),
      ) +
      blocoResumo(
        "atencao",
        `${grupos.sem_confirmacao.length} sem confirmação da Sefin`,
        grupos.sem_confirmacao.map(([n, r]) => `<li>${rotuloNota(n)}: ${esc(r.mensagem ?? "")}</li>`),
        "<p>Repetir o cancelamento dessas notas é seguro: o pedido repetido não duplica o evento.</p>",
      ),
  );
}
