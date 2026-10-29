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

// Local da prestação (emissão): o CEP preenche o município.
ligarConsultaCep(
  $("#local-cep"),
  (d) => {
    if (d.municipio) $("#local-prestacao").value = d.municipio;
  },
  $("#local-cidade"),
);

$("#local-prestacao").addEventListener("input", () => {
  $("#local-cidade").textContent = "";
});

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

// ---------------------------------------------------------------- busca de CEP pelo nome da rua

const dlgBuscaCep = $("#dlg-buscar-cep");
const formBuscaCep = $("#form-buscar-cep");
const erroBuscaCep = $("#erro-buscar-cep");
const resultadoBuscaCep = $("#resultado-buscar-cep");
let campoCepAlvo = null;
let buscaEmCurso = 0;

/** Mostra a lista de ruas encontradas; escolher uma põe o CEP no campo e deixa a consulta de CEP preencher o resto. */
function mostrarRuas(ruas) {
  resultadoBuscaCep.replaceChildren();
  if (!ruas.length) {
    const p = document.createElement("p");
    p.className = "vazio";
    p.textContent = "Nenhum CEP encontrado. Confira a cidade e tente menos palavras do nome da rua.";
    resultadoBuscaCep.append(p);
    return;
  }
  for (const r of ruas) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "botao item-cep";
    const rua = document.createElement("strong");
    rua.textContent = r.logradouro || "CEP geral da cidade";
    const detalhe = document.createElement("small");
    detalhe.textContent = [r.bairro, `${r.municipio_nome ?? ""}/${r.uf ?? ""}`, `CEP ${r.cep.slice(0, 5)}-${r.cep.slice(5)}`]
      .filter(Boolean)
      .join(" · ");
    b.append(rua, detalhe);
    b.onclick = () => {
      dlgBuscaCep.close();
      campoCepAlvo.value = r.cep;
      campoCepAlvo.dispatchEvent(new Event("change", { bubbles: true }));
    };
    resultadoBuscaCep.append(b);
  }
  if (ruas.length >= 50) {
    const p = document.createElement("p");
    p.className = "ajuda";
    p.textContent = "Mostrando os 50 primeiros: escreva mais do nome da rua para reduzir a lista.";
    resultadoBuscaCep.append(p);
  }
}

formBuscaCep.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  if (ev.submitter?.value === "cancelar") return dlgBuscaCep.close();
  const dados = new FormData(formBuscaCep);
  const botao = $('button[value="buscar"]', formBuscaCep);
  const minha = ++buscaEmCurso;
  erroBuscaCep.hidden = true;
  botao.disabled = true;
  botao.textContent = "Buscando...";
  try {
    const ruas = await chamar("buscar_cep_por_endereco", {
      uf: dados.get("uf"),
      cidade: dados.get("cidade"),
      logradouro: dados.get("rua"),
    });
    if (minha === buscaEmCurso) mostrarRuas(ruas);
  } catch (e) {
    resultadoBuscaCep.replaceChildren();
    erroBuscaCep.textContent = String(e);
    erroBuscaCep.hidden = false;
  } finally {
    botao.disabled = false;
    botao.textContent = "Buscar";
  }
});

for (const botao of document.querySelectorAll("[data-buscar-cep]")) {
  botao.addEventListener("click", () => {
    campoCepAlvo = document.getElementById(botao.dataset.buscarCep);
    erroBuscaCep.hidden = true;
    resultadoBuscaCep.replaceChildren();
    // UF e cidade ficam da última busca; a rua começa vazia.
    formBuscaCep.elements.rua.value = "";
    dlgBuscaCep.showModal();
    formBuscaCep.elements[formBuscaCep.elements.uf.value && formBuscaCep.elements.cidade.value ? "rua" : "uf"].focus();
  });
}
