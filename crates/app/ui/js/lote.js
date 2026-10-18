// Lote do mês: emite as recorrências pendentes de um mês de referência.

import { $, $$, NOME_SITUACAO, esc, estado, invoke, moeda, somarValores, toast } from "./util.js";
import { definirMotivo, detalheLinha, linhaVazia, selecaoEmTabela } from "./tabela.js";
import { confirmar } from "./formularios.js";
import { anunciar } from "./componentes.js";
import { atualizando } from "./atualizacao.js";

let loteCarregado = [];
export let loteEmAndamento = false;

const podeEmitirNoLote = (r) => !r.erros.length && (!r.no_mes || r.no_mes.libera);
export const ambienteDe = (empresaId) => estado.empresas.find((e) => e.id === empresaId)?.ambiente;
const selecaoLote = selecaoEmTabela({
  lista: "#lista-lote",
  itens: () => loteCarregado,
  botao: "#lote-emitir",
  rotulo: "Emitir selecionadas",
  todos: "#lote-todos",
  marcar: "#lote-marcar",
  emAndamento: () => loteEmAndamento,
});
const esperar = (ms) => new Promise((r) => setTimeout(r, ms));

export async function carregarLote() {
  // Durante a emissão a lista mostra o andamento; recarrega ao terminar.
  if (loteEmAndamento) return;
  const mes = $("#mes-lote").value;
  if (!mes) {
    $("#lista-lote").innerHTML = `<tr><td colspan="6" class="vazio">Escolha o mês de referência.</td></tr>`;
    return;
  }
  const soEmpresa = $("#lote-so-empresa").checked && estado.empresa;
  loteCarregado = await invoke("listar_recorrencias", { empresaId: soEmpresa ? estado.empresa.id : null, mes });
  desenharLote();
}

/** Carimbo da situação no lote e o motivo (vai para o detalhe da linha). */
function situacaoLote(r) {
  const t = r.no_mes;
  if (t && !t.libera) {
    const nota = t.numero_nfse ? ` nº ${esc(t.numero_nfse)}` : "";
    return { selo: `<span class="selo ${esc(t.situacao)}">${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)}${nota}</span>` };
  }
  const anterior = t ? `<p>Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</p>` : "";
  if (r.erros.length) {
    return {
      selo: `<span class="selo rejeitada">Não pode emitir</span>`,
      motivo: `<p>${r.erros.map(esc).join("<br>")}</p>${anterior}`,
      erro: true,
    };
  }
  return { selo: `<span class="selo pendente">Pendente</span>`, motivo: anterior };
}

/** Empresa na linha do lote/planilha: o nome corta com reticências e o carimbo de homologação fica inteiro. */
export function celulaEmpresa(nome, empresaId) {
  const emHomologacao = ambienteDe(empresaId) === "homologacao";
  const hom = emHomologacao ? `<span class="selo homologacao">Homologação</span>` : "";
  const titulo = emHomologacao ? `${nome} (homologação, sem valor fiscal)` : nome;
  return `<div class="celula-flex"><span class="corta" title="${esc(titulo)}">${esc(nome ?? "—")}</span>${hom}</div>`;
}

function desenharLote() {
  const corpo = $("#lista-lote");
  $("#lote-todos").checked = false;
  $("#rodape-lote").innerHTML = "";
  if (!loteCarregado.length) {
    corpo.innerHTML = linhaVazia(
      7,
      `Nenhuma recorrência cadastrada${$("#lote-so-empresa").checked ? " nesta empresa" : ""}.`,
      { ir: "recorrentes", texto: "Cadastrar recorrência" },
    );
    selecaoLote.atualizar();
    return;
  }
  corpo.innerHTML = loteCarregado
    .map((r, i) => {
      const s = situacaoLote(r);
      const alvo = `${r.empresa} para ${r.tomador ?? "nota sem tomador"}`;
      const det = detalheLinha({
        id: `det-lote-${i}`,
        colunas: 7,
        alvo,
        itens: [
          ["Empresa", esc(r.empresa)],
          ["Tomador", esc(r.tomador ?? "Não identificado")],
          ["Descrição", esc(r.descricao_no_mes)],
        ],
        motivo: s.motivo ?? "",
        erro: !!s.erro,
        aberta: !!s.erro,
      });
      return `<tr data-i="${i}">
        <td><input type="checkbox" data-i="${i}" ${podeEmitirNoLote(r) ? "" : "disabled"} aria-label="${esc(`Marcar ${alvo}`)}"></td>
        <td>${det.botao}</td>
        <td>${celulaEmpresa(r.empresa, r.empresa_id)}</td>
        <td title="${esc(r.tomador)}">${esc(r.tomador ?? "—")}</td>
        <td class="sec-1" title="${esc(r.descricao_no_mes)}">${esc(r.descricao_no_mes)}</td>
        <td class="num">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}</td>
        <td data-situacao>${s.selo}</td>
      </tr>${det.linha}`;
    })
    .join("");
  const pendentes = loteCarregado.filter(podeEmitirNoLote);
  const total = somarValores(pendentes, (r) => r.valor_efetivo);
  const comErro = loteCarregado.filter((r) => r.erros.length).length;
  $("#rodape-lote").innerHTML = `<tr><td colspan="7">${loteCarregado.length} recorrência(s) · ${pendentes.length}
    pendente(s), total <span class="dado">R$ ${moeda(total)}</span>${
      comErro ? ` · ${comErro} com erro` : ""
    }</td></tr>`;
  selecaoLote.atualizar();
}

$("#mes-lote").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));
$("#lote-so-empresa").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));

/**
 * Andamento do lote/planilha: contagem, um segmento por nota na ordem da fila e o total por
 * situação em carimbos (a barra sozinha diria só por cor). É uma progressbar para o leitor de tela.
 */
function desenharAndamento(el, total, situacoes, atual) {
  const feitas = situacoes.length;
  const conta = (s) => situacoes.filter((x) => x === s).length;
  const segmentos = Array.from({ length: total }, (_, i) => {
    const s = situacoes[i];
    const classe = s === "ok" ? "ok" : s === "erro" ? "erro" : s === "incerta" ? "incerta" : i === feitas ? "atual" : "";
    return `<span class="${classe}"></span>`;
  }).join("");
  const carimbos = [
    [conta("ok"), "autorizada", "autorizada(s)"],
    [conta("erro"), "rejeitada", "não emitida(s)"],
    [conta("incerta"), "incerta", "sem resposta"],
    [total - feitas - 1, "pendente", "na fila"],
  ]
    .filter(([n]) => n > 0)
    .map(([n, classe, texto]) => `<span class="selo ${classe}">${n} ${texto}</span>`)
    .join("");
  el.innerHTML = `<div class="cartao andamento" role="progressbar" aria-label="Andamento da emissão"
      aria-valuemin="0" aria-valuemax="${total}" aria-valuenow="${feitas}"
      aria-valuetext="${feitas} de ${total} concluídas; emitindo ${esc(atual)}">
    <div class="contagem"><span>Emitindo <b>${esc(atual)}</b>. Não feche o programa.</span>
      <span class="dado">${feitas + 1} / ${total}</span></div>
    <div class="barra${total > 60 ? " continua" : ""}" aria-hidden="true">${segmentos}</div>
    <div class="legenda-lote">${carimbos}</div>
  </div>`;
}

/**
 * Emite os itens em sequência, mostrando o andamento em `andamento` e o resultado na célula de
 * cada linha (`celulaDe(item)`). `emitirUm(item)` chama o comando; com limite de requisições da
 * Sefin, espera e tenta mais uma vez. Devolve o resumo em HTML.
 */
async function emitirEmSequencia({ itens, andamento, celulaDe, descrever, emitirUm }) {
  const contagem = { autorizada: 0, falha: 0, incerta: 0 };
  const situacoes = []; // "ok" | "erro" | "incerta", na ordem da fila
  // As células são achadas antes de começar: se a lista for redesenhada no meio, a fila continua
  // (escrevendo em células que já saíram da tela) em vez de parar com a nota em curso sem registro.
  const sobra = document.createElement("td");
  const celulas = new Map(itens.map((item) => [item, celulaDe(item) ?? sobra]));
  for (const item of itens) {
    desenharAndamento(andamento, itens.length, situacoes, descrever(item));
    const celula = celulas.get(item);
    celula.innerHTML = `<span class="selo enviando">Enviando...</span>`;
    let resultado;
    for (let tentativa = 1; !resultado; tentativa++) {
      try {
        resultado = { ok: true, res: await emitirUm(item) };
      } catch (e) {
        // só o 429 da Sefin (texto de `ErroSefin::LimiteRequisicoes`): uma rejeição que fale em
        // "limite" não é reenviada
        if (tentativa === 1 && String(e).includes("(HTTP 429)")) {
          celula.innerHTML = `<span class="selo enviando">Aguardando a Sefin...</span>`;
          await esperar(15000);
          continue;
        }
        resultado = { ok: false, erro: String(e) };
      }
    }
    // o carimbo fica na célula; avisos e motivos vão para o detalhe da linha (aberto quando não emitiu)
    const linha = celula.closest("tr");
    const motivo = (html, opcoes) => linha && definirMotivo(linha, html, opcoes);
    if (resultado.ok && resultado.res.autorizada) {
      contagem.autorizada++;
      situacoes.push("ok");
      celula.innerHTML = `<span class="selo autorizada">Autorizada nº ${esc(resultado.res.numero ?? "")}</span>`;
      motivo(resultado.res.avisos.map((a) => `<p>${esc(a)}</p>`).join(""));
    } else if (resultado.ok) {
      contagem.incerta++;
      situacoes.push("incerta");
      celula.innerHTML = `<span class="selo incerta">Sem resposta</span>`;
      motivo(`<p>${esc(resultado.res.motivo)}</p>`, { abrir: true });
      anunciar(`${descrever(item)}: sem resposta da Sefin.`);
    } else {
      contagem.falha++;
      situacoes.push("erro");
      celula.innerHTML = `<span class="selo rejeitada">Não emitida</span>`;
      motivo(`<p>${esc(resultado.erro)}</p>`, { erro: true, abrir: true });
      anunciar(`${descrever(item)}: não emitida.`);
    }
  }
  andamento.innerHTML = "";
  toast(`Concluído: ${contagem.autorizada} autorizada(s).`);
  return (
    `<div class="aviso ${contagem.falha || contagem.incerta ? "atencao" : "ok"}">Concluído: ` +
    `${contagem.autorizada} autorizada(s)` +
    (contagem.falha ? `, ${contagem.falha} não emitida(s) — o motivo aparece embaixo de cada linha` : "") +
    (contagem.incerta ? `, ${contagem.incerta} sem resposta — confira em "Notas emitidas" (Verificar)` : "") +
    ".</div>"
  );
}

/**
 * Roda a fila travando os controles que redesenham a lista (mês, filtros, reler). Erro inesperado
 * vira o resumo, e os controles e o "em andamento" sempre voltam (senão o botão ficava preso).
 */
export async function emitirTravando(controles, marcarAndamento, opcoes) {
  if (atualizando) return `<div class="aviso erro">${esc("A atualização do sistema está em andamento; emita depois que ele abrir de novo.")}</div>`;
  marcarAndamento(true);
  for (const c of controles) $(c).disabled = true;
  try {
    return await emitirEmSequencia(opcoes);
  } catch (e) {
    return `<div class="aviso erro">A emissão parou: ${esc(String(e))}. Confira a situação das notas em "Notas emitidas".</div>`;
  } finally {
    for (const c of controles) $(c).disabled = false;
    marcarAndamento(false);
  }
}

/** Confirmação antes de emitir em lote: quantidade, total e quantas em Produção. */
export function confirmarEmissao(quantidade, total, producao, referencia) {
  return confirmar(
    `Emitir ${quantidade} nota(s)${referencia ? ` com referência ${referencia}` : ""}, total R$ ${moeda(total)}?` +
      (producao ? `\n\n${producao} em PRODUÇÃO (valor fiscal).` : ""),
  );
}

$("#lote-emitir").addEventListener("click", async () => {
  const itens = selecaoLote.marcados();
  const mes = $("#mes-lote").value;
  if (!itens.length || !mes) return;
  const [ano, m] = mes.split("-");
  const producao = itens.filter((r) => ambienteDe(r.empresa_id) === "producao").length;
  const total = somarValores(itens, (r) => r.valor_efetivo);
  if (!(await confirmarEmissao(itens.length, total, producao, `${m}/${ano}`))) return;

  for (const c of $$("#lista-lote input[type=checkbox]")) c.disabled = true;
  const andamento = (sim) => {
    loteEmAndamento = sim;
    selecaoLote.atualizar();
  };
  const resumo = await emitirTravando(["#mes-lote", "#lote-so-empresa"], andamento, {
    itens,
    andamento: $("#lote-andamento"),
    celulaDe: (r) => $(`#lista-lote tr[data-i="${loteCarregado.indexOf(r)}"] [data-situacao]`),
    descrever: (r) => `${r.empresa} → ${r.tomador ?? "sem tomador"}`,
    emitirUm: (r) => invoke("emitir_recorrencia", { recorrenciaId: r.id, mes }),
  });
  // Recarrega para mostrar a situação gravada (as rejeitadas voltam a ficar pendentes).
  await carregarLote().catch((e) => toast(String(e), true));
  $("#lote-andamento").innerHTML = resumo;
});
