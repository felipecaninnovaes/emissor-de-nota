// Tomadores: cadastro, e-mails e retenções por empresa.

import { $, chamar, esc, estado, formatarDocumento, invoke, soDigitos, toast, vazio } from "./util.js";
import { acoesLinha, linhaVazia } from "./tabela.js";
import { ErroCampo, abrirDialogo, preencherForm } from "./formularios.js";
import { vincularTomador } from "./componentes.js";

let tomadoresCarregados = [];

export async function carregarTomadores() {
  const corpo = $("#lista-tomadores");
  if (!estado.empresa) {
    corpo.innerHTML = linhaVazia(5, "Nenhuma empresa selecionada.", { ir: "empresas", texto: "Cadastrar empresa" });
    return;
  }
  tomadoresCarregados = await invoke("buscar_tomadores", {
    empresaId: estado.empresa.id,
    termo: $("#busca-tomadores").value,
  });
  const termo = $("#busca-tomadores").value.trim();
  if (termo) {
    const outros = await invoke("buscar_tomadores_de_outras_empresas", { empresaId: estado.empresa.id, termo });
    tomadoresCarregados.push(...outros.map((t) => ({ ...t, de_outra_empresa: true })));
  }
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
    : linhaVazia(5, "Nenhum tomador encontrado (a lista mostra até 20; use a busca).", {
        clicar: "#novo-tomador",
        texto: "Novo tomador",
      });
}

let buscaTomadores = 0;
$("#busca-tomadores").addEventListener("input", () => {
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
    const pessoaId = await chamar("salvar_tomador", { empresaId, tomador, emails, retemFederais, retemIss });
    toast("Tomador salvo.");
    depois?.({ pessoa_id: pessoaId, tomador, emails, retem_federais: retemFederais, retem_iss: retemIss });
  });
  if (!dto && documento) {
    // Vindo da planilha: já preenche o documento e, se for CNPJ, consulta os dados.
    const cpf = documento.length === 11;
    form.elements.tipo.value = cpf ? "cpf" : "cnpj";
    form.elements.documento.value = formatarDocumento(cpf ? { cpf: documento } : { cnpj: documento });
    form.elements.documento.dispatchEvent(new Event("change"));
  }
}
