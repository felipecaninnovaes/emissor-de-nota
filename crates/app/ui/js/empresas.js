// Empresas: cadastro, certificado e numeração.

import { $, NOME_AMBIENTE, chamar, esc, estado, mostrarErro, soDigitos, toast, vazio } from "./util.js";
import { acoesLinha, linhaVazia } from "./tabela.js";
import { ErroCampo, abrirDialogo, confirmar, preencherForm } from "./formularios.js";
import { carregarEmpresas } from "./navegacao.js";

export async function renderizarEmpresas() {
  const corpo = $("#lista-empresas");
  corpo.innerHTML = estado.empresas.length
    ? estado.empresas
        .map((e, i) => {
          const c = e.certificado;
          // validade em uma linha; titular no title; carimbo só quando vence em até 30 dias
          let cert = `<span class="selo rejeitada">Sem certificado</span>`;
          if (c) {
            const d = c.dias_para_vencer;
            const carimbo =
              d < 0
                ? `<span class="selo rejeitada">Vencido</span>`
                : d <= 30
                  ? `<span class="selo incerta">${d} ${d === 1 ? "dia" : "dias"}</span>`
                  : "";
            cert = `<span class="dado">${esc(c.valido_ate)}</span> ${carimbo}`;
          }
          return `<tr>
            <td class="dado sec-2">${esc(e.cnpj_formatado)}</td>
            <td title="${esc(`${e.nome} — ${e.cnpj_formatado}`)}">${esc(e.nome)}</td>
            <td><span class="selo ${e.ambiente}">${NOME_AMBIENTE[e.ambiente]}</span></td>
            <td class="dado sec-1">${esc(e.serie)}</td>
            <td title="${esc(c ? `Titular: ${c.titular}` : "Importe o certificado A1 da empresa")}">${cert}</td>
            <td>${acoesLinha(
              [
                { acao: "editar", texto: "Editar" },
                { acao: "certificado", texto: "Certificado" },
                { acao: "numeracao", texto: "Numeração / IM" },
                { acao: "excluir", texto: "Excluir", perigo: true },
              ],
              i,
              e.nome,
            )}</td>
          </tr>`;
        })
        .join("")
    : linhaVazia(6, "Nenhuma empresa cadastrada.", { clicar: "#nova-empresa", texto: "Nova empresa" });
}

$("#lista-empresas").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const e = estado.empresas[Number(b.dataset.i)];
  if (b.dataset.acao === "certificado") importarCertificado(e);
  else if (b.dataset.acao === "excluir") excluirEmpresa(e);
  else if (b.dataset.acao === "editar") editarEmpresa(e);
  else editarNumeracao(e);
});

async function excluirEmpresa(empresa) {
  const pergunta =
    `Excluir ${empresa.nome} (${empresa.cnpj_formatado})?\n\n` +
    "Sem notas emitidas, o cadastro, o certificado e os serviços são apagados.\n" +
    "Com notas emitidas, a empresa só é arquivada e as notas continuam guardadas.";
  if (!(await confirmar(pergunta))) return;
  try {
    toast(await chamar("excluir_empresa", { empresaId: empresa.id }));
    if (estado.empresa?.id === empresa.id) estado.empresa = null;
    await carregarEmpresas();
    await renderizarEmpresas();
  } catch (e) {
    toast(String(e), true);
  }
}

$("#nova-empresa").addEventListener("click", () => editarEmpresa(null));

/** Cadastro (`empresa` nulo) ou edição de empresa no mesmo diálogo. */
function editarEmpresa(empresa) {
  const dlg = $("#dlg-empresa");
  const form = $("form", dlg);
  form.reset();
  $("[data-aviso-cnpj]", dlg).hidden = true;
  $("#titulo-empresa").textContent = empresa ? "Editar empresa" : "Nova empresa";
  form.elements.cnpj.readOnly = !!empresa;
  $("[data-consultar-cnpj]", dlg).textContent = "Consultar";
  if (empresa) {
    preencherForm(form, {
      cnpj: empresa.cnpj_formatado,
      nome: empresa.nome,
      im: empresa.inscricao_municipal,
      municipio: empresa.municipio_emissor,
      serie: empresa.serie,
      simples: empresa.regime.simples,
      apuracao: empresa.regime.apuracao_simples ?? "",
      regime_especial: String(empresa.regime.regime_especial ?? 0),
      ambiente: empresa.ambiente,
      telefone: empresa.telefone,
      email: empresa.email,
    });
  }
  abrirDialogo(dlg, $("#erro-empresa"), async (d) => {
    if (d.simples === "me_epp" && !d.apuracao) throw new ErroCampo("apuracao", "Para ME/EPP optante, escolha a forma de apuração.");
    const regime = { simples: d.simples, regime_especial: Number(d.regime_especial) };
    if (d.simples === "me_epp") regime.apuracao_simples = d.apuracao;
    const dados = {
      cnpj: empresa ? empresa.cnpj : d.cnpj.replace(/[^0-9A-Za-z]/g, "").toUpperCase(),
      nome: d.nome.trim(),
      inscricao_municipal: vazio(d.im),
      municipio_emissor: soDigitos(d.municipio),
      regime,
      ambiente: d.ambiente,
      serie: d.serie,
      telefone: vazio(soDigitos(d.telefone)),
      email: vazio(d.email),
    };
    const vaiParaProducao = d.ambiente === "producao" && empresa?.ambiente !== "producao";
    const mudaEmissao = empresa && (d.ambiente !== empresa.ambiente || d.serie.trim() !== empresa.serie);
    if (vaiParaProducao && !(await confirmar(`${empresa ? "Passar" : "Cadastrar"} ${d.nome.trim()} em PRODUÇÃO? As notas emitidas terão valor fiscal.`))) {
      throw "Não confirmado.";
    }
    if (!empresa) {
      const id = await chamar("cadastrar_empresa", { empresa: dados });
      toast("Empresa cadastrada. Agora importe o certificado digital.");
      estado.empresa = { id };
      await carregarEmpresas();
      await renderizarEmpresas();
      const nova = estado.empresas.find((x) => x.id === id);
      if (nova) setTimeout(() => importarCertificado(nova), 0);
      return;
    }
    await chamar("atualizar_empresa", { empresaId: empresa.id, empresa: dados });
    toast(
      mudaEmissao
        ? `Empresa agora emite em ${NOME_AMBIENTE[d.ambiente]}, série ${d.serie.trim()}. Confira o próximo número e a IM em "Numeração / IM".`
        : "Empresa atualizada.",
    );
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}

function importarCertificado(empresa) {
  const dlg = $("#dlg-certificado");
  $("form", dlg).reset();
  $("#cert-empresa").textContent = `${empresa.nome} — ${empresa.cnpj_formatado}`;
  abrirDialogo(dlg, $("#erro-certificado"), async (d, form) => {
    const arquivo = form.elements.arquivo.files[0];
    if (!arquivo) throw "Escolha o arquivo do certificado.";
    const pfx = Array.from(new Uint8Array(await arquivo.arrayBuffer()));
    const info = await chamar("importar_certificado", { empresaId: empresa.id, pfx, senha: d.senha });
    form.elements.senha.value = "";
    toast(`Certificado de ${info.titular} importado (válido até ${info.valido_ate}).`);
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}

function editarNumeracao(empresa) {
  const dlg = $("#dlg-numeracao");
  const form = $("form", dlg);
  form.reset();
  $("#num-empresa").textContent =
    `${empresa.nome} — série ${empresa.serie} em ${NOME_AMBIENTE[empresa.ambiente]}` +
    (empresa.inscricao_municipal ? ` — IM ${empresa.inscricao_municipal}` : " — sem inscrição municipal cadastrada");
  preencherForm(form, {
    proximo: "",
    im_prod: empresa.im_na_dps_producao,
    im_hom: empresa.im_na_dps_homologacao,
  });
  form.elements.proximo.placeholder = "deixe em branco para não mudar";
  const botaoSefin = $("[data-ajustar-sefin]", dlg);
  botaoSefin.onclick = async () => {
    botaoSefin.disabled = true;
    botaoSefin.textContent = "Consultando...";
    try {
      const msg = await chamar("ajustar_numeracao", { empresaId: empresa.id });
      toast(msg);
      $("#num-empresa").textContent = msg;
    } catch (e) {
      mostrarErro($("#erro-numeracao"), e);
    } finally {
      botaoSefin.disabled = false;
      botaoSefin.textContent = "Consultar na Sefin";
    }
  };
  abrirDialogo(dlg, $("#erro-numeracao"), async (d) => {
    if (vazio(d.proximo)) {
      const proximo = Number(d.proximo);
      if (!Number.isInteger(proximo) || proximo < 1) throw new ErroCampo("proximo", "Número inteiro, maior que zero.");
      if (!(await confirmar(`Próxima DPS da série ${empresa.serie} será a nº ${proximo}. Confirma?`))) throw "Não confirmado.";
      await chamar("definir_proximo_numero", {
        empresaId: empresa.id,
        ambiente: empresa.ambiente,
        serie: empresa.serie,
        proximo,
      });
    }
    if (d.im_prod !== empresa.im_na_dps_producao) {
      await chamar("definir_im_na_dps", { empresaId: empresa.id, ambiente: "producao", informar: d.im_prod });
    }
    if (d.im_hom !== empresa.im_na_dps_homologacao) {
      await chamar("definir_im_na_dps", { empresaId: empresa.id, ambiente: "homologacao", informar: d.im_hom });
    }
    toast("Configuração salva.");
    return async () => {
      await carregarEmpresas();
      await renderizarEmpresas();
    };
  });
}
