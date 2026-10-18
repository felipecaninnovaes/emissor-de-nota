// Notas emitidas: lista, e-mail, cancelamento, refazer e copiar.

import { $, NOME_SITUACAO, chamar, esc, estado, invoke, moeda, somarValores, toast, vazio } from "./util.js";
import { acoesLinha, detalheLinha, linhaVazia } from "./tabela.js";
import { ErroCampo, abrirDialogo } from "./formularios.js";
import { irPara } from "./navegacao.js";
import { abrirNota, aplicarServico, escolherTomador, limparEmissao, preencherObra } from "./emitir.js";
import { atualizando } from "./atualizacao.js";

let notasCarregadas = [];

export async function carregarNotas() {
  const corpo = $("#lista-notas");
  $("#rodape-notas").innerHTML = "";
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(8, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  notasCarregadas = await invoke("listar_notas", { empresaId: estado.empresa.id, mes: vazio($("#mes-notas").value) });
  if (!notasCarregadas.length) {
    corpo.innerHTML = linhaVazia(8, "Nenhuma nota neste período.", { ir: "emitir", texto: "Emitir nota" });
    return;
  }
  corpo.innerHTML = notasCarregadas
    .map((n, i) => {
      // a ação mais usada em cada situação fica visível; as outras vão para "Mais"
      const acoes = [];
      if (n.situacao === "rejeitada") acoes.push({ acao: "refazer", texto: "Corrigir e emitir" });
      if (n.situacao === "incerta" || n.situacao === "enviando") acoes.push({ acao: "verificar", texto: "Verificar" });
      if (n.nfse_id) {
        acoes.push({ acao: "pdf", texto: "PDF" }, { acao: "pasta", texto: "Pasta" });
        if (n.situacao === "autorizada" || n.situacao === "cancelada") {
          acoes.push({ acao: "email", texto: "E-mail" }, { acao: "copiar", texto: "Copiar" });
        }
        if (n.situacao === "autorizada") {
          acoes.push({ acao: "conferir", texto: "Conferir na Sefin" });
          acoes.push({ acao: "cancelar", texto: "Cancelar nota", perigo: true });
        }
      }
      const alvo = n.numero_nfse ? `nota ${n.numero_nfse}` : `DPS ${n.serie}/${n.numero_dps}`;
      const motivo = n.erro && n.situacao !== "autorizada" ? esc(n.erro) : "";
      const det = detalheLinha({
        id: `det-nota-${i}`,
        colunas: 8,
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
  $("#rodape-notas").innerHTML = `<tr><td colspan="8">${notasCarregadas.length} nota(s) · ${autorizadas.length}
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
  $("#info-compl").value = r.informacoes_complementares ?? "";
  preencherObra(r.obra);
  const avisos = [aviso];
  if (!servicoAchado) avisos.push("Nenhum serviço do cadastro tem o mesmo código: escolha o serviço.");
  if (n.tomador && !r.tomador) avisos.push(`O tomador "${n.tomador}" não foi encontrado no cadastro desta empresa.`);
  const res = $("#resultado");
  res.innerHTML = `<div class="aviso atencao">${avisos.map(esc).join("<br>")}
    <br>${orientacao} A nota sai com o próximo número da série.</div>`;
  res.hidden = false;
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
    n.situacao === "cancelada" ? " (CANCELADA: o e-mail avisa o cancelamento)" : ""
  }`;
  form.elements.para.value = para.join("; ");
  abrirDialogo(dlg, $("#erro-email"), async (d) => {
    toast(await chamar("enviar_email", { nfseId: n.nfse_id, para: d.para }));
    return carregarNotas;
  });
}

export let cancelamentoEmCurso = false;

function cancelarNota(n) {
  const dlg = $("#dlg-cancelar");
  $("#cancelar-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"} — R$ ${moeda(n.valor)}`;
  $("form", dlg).reset();
  abrirDialogo(dlg, $("#erro-cancelar"), async (d) => {
    const justificativa = d.justificativa.trim();
    if (justificativa.length < 15) throw new ErroCampo("justificativa", "Escreva pelo menos 15 caracteres.");
    if (atualizando) throw "A atualização do sistema está em andamento; cancele depois que ele abrir de novo.";
    cancelamentoEmCurso = true;
    try {
      await chamar("cancelar", { nfseId: n.nfse_id, motivo: d.motivo, justificativa });
    } finally {
      cancelamentoEmCurso = false;
    }
    toast(`NFS-e ${n.numero_nfse ?? ""} cancelada.`);
    return carregarNotas;
  });
}
