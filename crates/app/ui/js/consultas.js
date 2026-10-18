// Consulta de CNPJ e de CEP para preencher cadastros.

import { $, chamar, soDigitos, toast, vazio } from "./util.js";

/**
 * Liga o botão "Consultar" (e a saída do campo, quando o nome ainda está vazio) à consulta
 * pública de CNPJ. `preencher(form, dados)` copia o que veio para o formulário.
 */
function ligarConsultaCnpj(dlg, campo, ehCnpj, preencher) {
  const form = $("form", dlg);
  const aviso = $("[data-aviso-cnpj]", dlg);
  const botao = $("[data-consultar-cnpj]", dlg);

  async function consultar(automatico) {
    const cnpj = form.elements[campo].value.replace(/[^0-9A-Za-z]/g, "");
    if (!ehCnpj(form)) return automatico || toast("A consulta é só para CNPJ.", true);
    if (cnpj.length !== 14) return automatico || toast("Informe os 14 caracteres do CNPJ.", true);
    aviso.hidden = true;
    botao.disabled = true;
    botao.textContent = "Consultando...";
    try {
      const d = await chamar("consultar_cnpj", { cnpj });
      preencher(form, d);
      if (d.situacao && d.situacao.toLowerCase() !== "ativa") {
        aviso.textContent = `Atenção: situação cadastral na Receita é "${d.situacao}".`;
        aviso.hidden = false;
      }
    } catch (e) {
      toast(String(e), true);
    } finally {
      botao.disabled = false;
      botao.textContent = "Consultar";
    }
  }

  botao.addEventListener("click", () => consultar(false));
  form.elements[campo].addEventListener("change", () => {
    if (vazio(form.elements.nome.value)) return; // não sobrescreve um cadastro já preenchido
    consultar(true);
  });
}

/** Copia só os valores que vieram da consulta (não apaga o que o operador já digitou). */
function copiarSeVeio(form, pares) {
  for (const [nome, valor] of pares) if (valor != null && valor !== "") form.elements[nome].value = valor;
}

ligarConsultaCnpj(
  $("#dlg-tomador"),
  "documento",
  (form) => form.elements.tipo.value === "cnpj",
  (form, d) => {
    copiarSeVeio(form, [
      ["nome", d.razao_social],
      ["cep", d.cep],
      ["municipio", d.municipio],
      ["logradouro", d.logradouro],
      ["numero", d.numero],
      ["complemento", d.complemento],
      ["bairro", d.bairro],
      ["telefone", d.telefone],
      ["email", d.email],
    ]);
    $("[data-municipio-nome]", form).textContent = d.municipio_nome ? `${d.municipio_nome}/${d.uf ?? ""}` : "";
  },
);

// ---------------------------------------------------------------- consulta de CEP

/**
 * Ao sair do campo de CEP com 8 dígitos, consulta o CEP e chama `preencher(dados)`.
 * `aviso` (opcional) mostra a cidade encontrada ou "consultando...".
 */
function ligarConsultaCep(campo, preencher, aviso) {
  campo.addEventListener("change", async () => {
    const cep = soDigitos(campo.value);
    if (cep.length !== 8) return;
    if (aviso) aviso.textContent = "Consultando o CEP...";
    try {
      const d = await chamar("consultar_cep", { cep });
      if (soDigitos(campo.value) !== cep) return; // o operador já trocou o CEP
      preencher(d);
      if (aviso) aviso.textContent = d.municipio_nome ? `${d.municipio_nome}/${d.uf ?? ""}` : "";
      if (!d.logradouro) toast("CEP geral da cidade: preencha logradouro e bairro.");
    } catch (e) {
      if (aviso) aviso.textContent = "";
      toast(String(e), true);
    }
  });
  campo.addEventListener("input", () => {
    if (aviso) aviso.textContent = "";
  });
}

for (const p of ["obra", "rec-obra"]) {
  ligarConsultaCep(
    $(`#${p}-cep`),
    (d) => {
      for (const [campo, valor] of [
        ["logradouro", d.logradouro],
        ["bairro", d.bairro],
        ["complemento", d.complemento],
      ]) {
        if (valor) $(`#${p}-${campo}`).value = valor;
      }
      $(d.logradouro ? `#${p}-numero` : `#${p}-logradouro`).focus();
    },
    $(`#${p}-cep-cidade`),
  );
}

ligarConsultaCep(
  $("#form-tomador").elements.cep,
  (d) => {
    const form = $("#form-tomador");
    copiarSeVeio(form, [
      ["municipio", d.municipio],
      ["logradouro", d.logradouro],
      ["complemento", d.complemento],
      ["bairro", d.bairro],
    ]);
    form.elements[d.logradouro ? "numero" : "logradouro"].focus();
  },
  $("#dlg-tomador [data-municipio-nome]"),
);

$("#form-tomador").elements.municipio.addEventListener("input", () => {
  $("#dlg-tomador [data-municipio-nome]").textContent = "";
});

ligarConsultaCnpj(
  $("#dlg-empresa"),
  "cnpj",
  () => true,
  (form, d) => {
    copiarSeVeio(form, [
      ["nome", d.razao_social],
      ["municipio", d.municipio],
      ["telefone", d.telefone],
      ["email", d.email],
    ]);
    if (d.mei) form.elements.simples.value = "mei";
    else if (d.optante_simples === true) form.elements.simples.value = "me_epp";
    else if (d.optante_simples === false) form.elements.simples.value = "nao_optante";
    if (d.municipio_nome) toast(`Município: ${d.municipio_nome}/${d.uf ?? ""}`);
  },
);
