// Recorrências: cadastro das notas que se repetem todo mês.

import {
  $,
  chamar,
  esc,
  estado,
  formatarDocumento,
  invoke,
  mesPadrao,
  moeda,
  toast,
  valorDigitado,
  vazio,
} from "./util.js";
import { acoesLinha, linhaVazia } from "./tabela.js";
import { abrirDialogo, confirmar } from "./formularios.js";
import { ligarBuscaTomador, mostrarTomadorEscolhido } from "./componentes.js";
import { CODIGOS_COM_OBRA, obraDigitada, preencherObra } from "./emitir.js";

let recorrenciasCarregadas = [];

export async function carregarRecorrencias() {
  const corpo = $("#lista-recorrencias");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(5, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  recorrenciasCarregadas = await invoke("listar_recorrencias", { empresaId: estado.empresa.id, mes: mesPadrao() });
  corpo.innerHTML = recorrenciasCarregadas.length
    ? recorrenciasCarregadas
        .map(
          (r, i) => `<tr>
          <td title="${esc(r.tomador)}">${esc(r.tomador ?? "—")}</td>
          <td class="sec-2" title="${esc(r.servico)}">${esc(r.servico)}</td>
          <td class="sec-1" title="${esc(r.descricao_no_mes)}">${esc(r.descricao ?? "(a do serviço)")}</td>
          <td class="num" title="${r.valor ? "" : "Valor do serviço"}">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}${
            r.valor ? "" : "*"
          }</td>
          <td>${acoesLinha(
            [
              { acao: "editar", texto: "Editar" },
              { acao: "remover", texto: "Remover", perigo: true },
            ],
            i,
            `recorrência de ${r.tomador ?? "nota sem tomador"}`,
          )}</td>
        </tr>`,
        )
        .join("")
    : linhaVazia(5, 'Nenhuma recorrência nesta empresa (também dá para usar "Tornar recorrente" depois de emitir).', {
        clicar: "#nova-recorrencia",
        texto: "Nova recorrência",
      });
}

$("#lista-recorrencias").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const r = recorrenciasCarregadas[Number(b.dataset.i)];
  if (b.dataset.acao === "editar") return editarRecorrencia(r);
  if (!(await confirmar(`Remover a recorrência de ${r.tomador ?? "nota sem tomador"}? As notas já emitidas não mudam.`))) return;
  try {
    await chamar("desativar_recorrencia", { id: r.id });
    await carregarRecorrencias();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#nova-recorrencia").addEventListener("click", () => editarRecorrencia(null));

const dlgRecorrencia = $("#dlg-recorrencia");
let tomadorRecorrencia = null; // { pessoa_id, nome } escolhido no diálogo

function escolherTomadorRecorrencia(t) {
  tomadorRecorrencia = t;
  mostrarTomadorEscolhido(
    {
      escolhido: $("[data-rec-escolhido]", dlgRecorrencia),
      busca: $("[data-rec-busca]", dlgRecorrencia),
      resultados: $("[data-rec-resultados]", dlgRecorrencia),
      termo: $("[data-rec-termo]", dlgRecorrencia),
      rotulo: $("[data-rec-tomador]", dlgRecorrencia),
    },
    t?.nome,
  );
}

ligarBuscaTomador($("[data-rec-termo]", dlgRecorrencia), $("[data-rec-resultados]", dlgRecorrencia), (t) =>
  escolherTomadorRecorrencia({
    pessoa_id: t.pessoa_id,
    nome: `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`,
  }),
);
$("[data-rec-trocar]", dlgRecorrencia).addEventListener("click", () => escolherTomadorRecorrencia(null));

/** Abre o diálogo de recorrência: `r` é uma existente; `base`, dados para preencher uma nova. */
export function editarRecorrencia(r, base = null) {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  if (!estado.servicos.length) return toast('Cadastre um serviço antes, em "Serviços".', true);
  const form = $("form", dlgRecorrencia);
  form.reset();
  $("#titulo-recorrencia").textContent = r ? "Editar recorrência" : "Nova recorrência";
  form.elements.servico.innerHTML = estado.servicos
    .map(
      (s) =>
        `<option value="${s.id}">${esc(s.nome)}${s.valor_sugerido ? ` (R$ ${moeda(s.valor_sugerido)})` : ""}</option>`,
    )
    .join("");
  const dados = r ?? base ?? {};
  escolherTomadorRecorrencia(dados.tomador_id ? { pessoa_id: dados.tomador_id, nome: dados.tomador } : null);
  if (dados.servico_id) form.elements.servico.value = String(dados.servico_id);
  form.elements.valor.value = dados.valor ? moeda(dados.valor) : "";
  form.elements.descricao.value = dados.descricao ?? "";
  form.elements.info.value = dados.informacoes_complementares ?? "";
  preencherObra(dados.obra ?? null, "rec-obra");
  const dicas = () => {
    const s = estado.servicos.find((x) => x.id === Number(form.elements.servico.value));
    form.elements.valor.placeholder = s?.valor_sugerido ? `${moeda(s.valor_sugerido)} (do serviço)` : "obrigatório";
    form.elements.descricao.placeholder = s?.descricao ?? "";
    $("#bloco-rec-obra").hidden = !CODIGOS_COM_OBRA.includes(s?.codigo_trib_nac);
  };
  form.elements.servico.onchange = dicas;
  dicas();
  abrirDialogo(dlgRecorrencia, $("#erro-recorrencia"), async (d) => {
    await chamar("salvar_recorrencia", {
      recorrencia: {
        id: r?.id ?? null,
        empresa_id: estado.empresa.id,
        tomador: tomadorRecorrencia?.pessoa_id ?? null,
        servico_id: Number(d.servico),
        valor: valorDigitado(d.valor),
        descricao: vazio(d.descricao),
        informacoes_complementares: vazio(d.info),
        obra: obraDigitada("rec-obra"),
      },
    });
    toast("Recorrência salva.");
    if (!$('section[data-tela="recorrentes"]').hidden) return carregarRecorrencias;
  });
}
