// Interface do emissor: liga as telas de index.html aos comandos Tauri (src/comandos.rs).
// Sem bundler: módulo ES puro. Argumentos de primeiro nível dos comandos vão em camelCase
// (convenção do Tauri); os campos dentro das structs seguem o serde (snake_case).

const invoke = window.__TAURI__.core.invoke;

const $ = (sel, raiz = document) => raiz.querySelector(sel);
const $$ = (sel, raiz = document) => [...raiz.querySelectorAll(sel)];

const estado = {
  empresas: [],
  empresa: null,
  servicos: [],
  tomador: null, // TomadorDto escolhido na tela de emitir
  pedido: null, // último pedido revisado
};

// ---------------------------------------------------------------- utilitários

function esc(texto) {
  return String(texto ?? "").replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
  );
}

function toast(texto, erro = false) {
  const el = document.createElement("div");
  el.className = "toast" + (erro ? " erro" : "");
  el.textContent = texto;
  $("#toasts").append(el);
  setTimeout(() => el.remove(), erro ? 8000 : 4000);
}

/** Chama um comando mostrando o cursor de espera; o erro volta como texto. */
async function chamar(comando, args) {
  document.body.classList.add("ocupado");
  try {
    return await invoke(comando, args);
  } finally {
    document.body.classList.remove("ocupado");
  }
}

function mostrarErro(el, erro) {
  el.textContent = String(erro);
  el.hidden = false;
}

/** "150.00" → "150,00" com separador de milhar. */
function moeda(valor) {
  if (valor == null || valor === "") return "";
  const [inteiro, frac = "00"] = String(valor).split(".");
  return inteiro.replace(/\B(?=(\d{3})+(?!\d))/g, ".") + "," + frac.padEnd(2, "0");
}

/** "1.500,00" → "1500,00" (o backend aceita vírgula decimal). Vazio → null. */
function valorDigitado(texto) {
  const t = String(texto ?? "").trim().replace(/\s|R\$/g, "");
  if (!t) return null;
  return t.includes(",") ? t.replace(/\./g, "") : t;
}

/** "11.48" → "11,48" para campos de percentual. */
function percentualTela(p) {
  return p == null ? "" : String(p).replace(".", ",");
}

const soDigitos = (t) => String(t ?? "").replace(/\D/g, "");
const vazio = (t) => (String(t ?? "").trim() === "" ? null : String(t).trim());

function formatarDocumento(doc) {
  if (!doc) return "";
  if (doc.cnpj) {
    const c = doc.cnpj;
    return `${c.slice(0, 2)}.${c.slice(2, 5)}.${c.slice(5, 8)}/${c.slice(8, 12)}-${c.slice(12)}`;
  }
  if (doc.cpf) {
    const c = doc.cpf;
    return `${c.slice(0, 3)}.${c.slice(3, 6)}.${c.slice(6, 9)}-${c.slice(9)}`;
  }
  return "";
}

function mesAnterior() {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function mesAtual() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

const NOME_AMBIENTE = { producao: "PRODUÇÃO", homologacao: "Homologação" };
const NOME_SITUACAO = {
  autorizada: "Autorizada",
  rejeitada: "Rejeitada",
  incerta: "Sem resposta",
  enviando: "Enviando",
  cancelada: "Cancelada",
  substituida: "Substituída",
};

/** Lê um <form> como objeto (checkbox → boolean). */
function lerForm(form) {
  const dados = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    dados[el.name] = el.type === "checkbox" ? el.checked : el.value;
  }
  return dados;
}

function preencherForm(form, dados) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in dados)) continue;
    if (el.type === "checkbox") el.checked = !!dados[el.name];
    else if (el.type !== "file") el.value = dados[el.name] ?? "";
  }
}

/**
 * Abre um <dialog> com formulário. `salvar(dados)` roda ao clicar no botão "salvar";
 * se lançar erro, a mensagem aparece em `erroEl` e o diálogo continua aberto.
 */
function abrirDialogo(dlg, erroEl, salvar) {
  const form = $("form", dlg);
  erroEl.hidden = true;
  form.onsubmit = async (ev) => {
    if (ev.submitter?.value !== "salvar") return; // Cancelar fecha normalmente
    ev.preventDefault();
    erroEl.hidden = true;
    const botao = ev.submitter;
    botao.disabled = true;
    try {
      await salvar(lerForm(form), form);
      dlg.close();
    } catch (e) {
      mostrarErro(erroEl, e);
    } finally {
      botao.disabled = false;
    }
  };
  dlg.showModal();
}

// ---------------------------------------------------------------- trava (senha mestra)

let cofreExiste = true;

async function iniciarTrava() {
  const s = await invoke("situacao_cofre");
  if (s.aberto) return entrar([]);
  cofreExiste = s.existe;
  if (!cofreExiste) {
    $("#trava-ajuda").textContent =
      "Primeiro uso: crie a senha mestra. Ela protege os certificados de todas as empresas.";
    $("#bloco-confirmacao").hidden = false;
    $("#senha2").required = true;
    $("#senha").autocomplete = "new-password";
    $("#botao-trava").textContent = "Criar e abrir";
  }
  $("#senha").focus();
}

$("#form-trava").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const erro = $("#erro-trava");
  erro.hidden = true;
  const senha = $("#senha").value;
  if (!cofreExiste) {
    if (senha.length < 8) return mostrarErro(erro, "A senha mestra precisa ter pelo menos 8 caracteres.");
    if (senha !== $("#senha2").value) return mostrarErro(erro, "As senhas não conferem.");
  }
  const botao = $("#botao-trava");
  botao.disabled = true;
  botao.textContent = "Abrindo...";
  try {
    const abertura = await chamar(cofreExiste ? "abrir_cofre" : "criar_cofre", { senha });
    $("#senha").value = "";
    $("#senha2").value = "";
    await entrar(abertura.pendencias);
  } catch (e) {
    mostrarErro(erro, e);
    botao.disabled = false;
    botao.textContent = cofreExiste ? "Abrir" : "Criar e abrir";
  }
});

async function entrar(pendencias) {
  $("#trava").hidden = true;
  $("#app").hidden = false;
  const alertas = $("#alertas");
  alertas.innerHTML = "";
  if (pendencias.length) {
    alertas.innerHTML = `<div class="aviso atencao"><strong>Envios anteriores verificados na Sefin:</strong>
      <ul>${pendencias.map((p) => `<li>${esc(p)}</li>`).join("")}</ul></div>`;
  }
  $("#mes-ref").value = mesAnterior();
  $("#mes-notas").value = mesAtual();
  await carregarEmpresas();
  await avisarVencimentos();
  if (!estado.empresas.length) {
    irPara("empresas");
    toast("Cadastre a primeira empresa para começar.");
  }
}

async function avisarVencimentos() {
  const lista = await invoke("certificados_vencendo", { dias: 30 }).catch(() => []);
  if (!lista.length) return;
  const itens = lista
    .map((v) => `<li>${esc(v.empresa)}: ${v.dias < 0 ? "vencido" : `vence em ${v.dias} dia(s)`} (${esc(v.valido_ate)})</li>`)
    .join("");
  $("#alertas").insertAdjacentHTML(
    "beforeend",
    `<div class="aviso atencao"><strong>Certificados vencendo:</strong><ul>${itens}</ul></div>`,
  );
}

// ---------------------------------------------------------------- navegação e empresa atual

function irPara(tela) {
  for (const b of $$("nav button[data-tela]")) b.classList.toggle("ativo", b.dataset.tela === tela);
  for (const s of $$("section[data-tela]")) s.hidden = s.dataset.tela !== tela;
  const carregar = {
    notas: carregarNotas,
    tomadores: carregarTomadores,
    servicos: carregarServicos,
    empresas: renderizarEmpresas,
    configuracoes: carregarConfiguracoes,
  }[tela];
  carregar?.().catch((e) => toast(String(e), true));
}

for (const b of $$("nav button[data-tela]")) b.addEventListener("click", () => irPara(b.dataset.tela));

async function carregarEmpresas() {
  estado.empresas = await invoke("listar_empresas");
  const sel = $("#empresa-atual");
  const anterior = estado.empresa?.id ?? Number(lerPreferencia("empresa"));
  sel.innerHTML = estado.empresas.length
    ? estado.empresas.map((e) => `<option value="${e.id}">${esc(e.nome)} — ${esc(e.cnpj_formatado)}</option>`).join("")
    : `<option value="">Nenhuma empresa cadastrada</option>`;
  const escolhida = estado.empresas.find((e) => e.id === anterior) ?? estado.empresas[0] ?? null;
  if (escolhida) sel.value = String(escolhida.id);
  await trocarEmpresa(escolhida);
}

function lerPreferencia(chave) {
  try {
    return localStorage.getItem("emissor." + chave);
  } catch {
    return null;
  }
}

function gravarPreferencia(chave, valor) {
  try {
    localStorage.setItem("emissor." + chave, valor);
  } catch {
    /* sem armazenamento local: apenas não lembra */
  }
}

$("#empresa-atual").addEventListener("change", (ev) => {
  const e = estado.empresas.find((x) => x.id === Number(ev.target.value)) ?? null;
  trocarEmpresa(e).catch((err) => toast(String(err), true));
});

async function trocarEmpresa(empresa) {
  estado.empresa = empresa;
  const selo = $("#selo-ambiente");
  const info = $("#info-certificado");
  if (!empresa) {
    selo.hidden = true;
    info.textContent = "";
    estado.servicos = [];
    renderizarServicosEmitir();
    return;
  }
  gravarPreferencia("empresa", empresa.id);
  selo.hidden = false;
  selo.className = "selo " + empresa.ambiente;
  selo.textContent = NOME_AMBIENTE[empresa.ambiente];
  const c = empresa.certificado;
  info.textContent = c
    ? `Certificado válido até ${c.valido_ate}${c.dias_para_vencer <= 30 ? ` (${c.dias_para_vencer} dias)` : ""}`
    : "Sem certificado digital";
  info.style.color = !c || c.dias_para_vencer <= 30 ? "var(--vermelho)" : "";
  limparEmissao();
  estado.servicos = await invoke("listar_servicos", { empresaId: empresa.id });
  renderizarServicosEmitir();
  // Recarrega a tela visível com a nova empresa.
  const visivel = $$("section[data-tela]").find((s) => !s.hidden)?.dataset.tela;
  if (visivel && visivel !== "emitir" && visivel !== "empresas") irPara(visivel);
}

// ---------------------------------------------------------------- emitir

function renderizarServicosEmitir() {
  const sel = $("#servico");
  sel.innerHTML = estado.servicos.length
    ? estado.servicos.map((s) => `<option value="${s.id}">${esc(s.nome)}</option>`).join("")
    : `<option value="">Cadastre um serviço em "Serviços"</option>`;
  aplicarServico();
}

function aplicarServico() {
  const s = estado.servicos.find((x) => x.id === Number($("#servico").value));
  if (!s) return;
  $("#descricao").value = s.descricao;
  if (s.valor_sugerido) $("#valor").value = moeda(s.valor_sugerido);
}

$("#servico").addEventListener("change", aplicarServico);

function limparEmissao() {
  escolherTomador(null);
  $("#valor").value = "";
  $("#info-compl").value = "";
  $("#mes-ref").value = mesAnterior();
  $("#revisao").hidden = true;
  $("#resultado").hidden = true;
}

function escolherTomador(t) {
  estado.tomador = t;
  $("#tomador-escolhido").hidden = !t;
  $("#busca-tomador").hidden = !!t;
  $("#resultados-tomador").hidden = true;
  $("#termo-tomador").value = "";
  if (t) $("#tomador-escolhido-texto").textContent = `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`;
}

$("#trocar-tomador").addEventListener("click", () => {
  escolherTomador(null);
  $("#termo-tomador").focus();
});

let buscaPendente = 0;
let resultadosBusca = [];
let foco = -1;

$("#termo-tomador").addEventListener("input", (ev) => {
  clearTimeout(buscaPendente);
  const termo = ev.target.value.trim();
  if (!termo || !estado.empresa) {
    $("#resultados-tomador").hidden = true;
    return;
  }
  buscaPendente = setTimeout(async () => {
    try {
      resultadosBusca = await invoke("buscar_tomadores", { empresaId: estado.empresa.id, termo });
    } catch (e) {
      return toast(String(e), true);
    }
    foco = resultadosBusca.length ? 0 : -1;
    desenharResultados();
  }, 200);
});

function desenharResultados() {
  const caixa = $("#resultados-tomador");
  caixa.hidden = false;
  caixa.innerHTML = resultadosBusca.length
    ? resultadosBusca
        .map(
          (t, i) => `<div data-i="${i}" class="${i === foco ? "foco" : ""}">${esc(t.tomador.nome)}
            <small>${esc(formatarDocumento(t.tomador.documento))}</small></div>`,
        )
        .join("")
    : `<div class="vazio">Nenhum tomador encontrado.</div>`;
}

$("#termo-tomador").addEventListener("keydown", (ev) => {
  if ($("#resultados-tomador").hidden || !resultadosBusca.length) return;
  if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    foco = (foco + (ev.key === "ArrowDown" ? 1 : -1) + resultadosBusca.length) % resultadosBusca.length;
    desenharResultados();
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    if (foco >= 0) escolherTomador(resultadosBusca[foco]);
  } else if (ev.key === "Escape") {
    $("#resultados-tomador").hidden = true;
  }
});

$("#resultados-tomador").addEventListener("mousedown", (ev) => {
  const item = ev.target.closest("[data-i]");
  if (item) escolherTomador(resultadosBusca[Number(item.dataset.i)]);
});

$("#termo-tomador").addEventListener("blur", () => setTimeout(() => ($("#resultados-tomador").hidden = true), 150));

$("#novo-tomador-emitir").addEventListener("click", (ev) => {
  ev.preventDefault();
  editarTomador(null, (salvo) => escolherTomador(salvo));
});

function montarPedido() {
  if (!estado.empresa) throw "Escolha a empresa.";
  const servicoId = Number($("#servico").value);
  if (!servicoId) throw "Escolha o serviço (cadastre um em \"Serviços\").";
  const valor = valorDigitado($("#valor").value);
  if (!valor) throw "Informe o valor.";
  return {
    empresa_id: estado.empresa.id,
    tomador: estado.tomador?.pessoa_id ?? null,
    servico_id: servicoId,
    valor,
    descricao: vazio($("#descricao").value),
    mes_referencia: vazio($("#mes-ref").value),
    informacoes_complementares: vazio($("#info-compl").value),
  };
}

$("#revisar").addEventListener("click", async () => {
  $("#resultado").hidden = true;
  let pedido;
  try {
    pedido = montarPedido();
  } catch (e) {
    return toast(String(e), true);
  }
  let previa;
  try {
    previa = await chamar("previa_dps", { pedido });
  } catch (e) {
    return toast(String(e), true);
  }
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
  aviso.innerHTML = partes.join("");
  const linhas = [
    ["Prestador", previa.prestador],
    ["Tomador", previa.tomador ?? "Não identificado"],
    ["Valor", "R$ " + moeda(previa.valor)],
    ["Competência", previa.competencia],
    ["Série da DPS", previa.serie],
    ["Descrição", previa.descricao],
  ];
  if (pedido.informacoes_complementares) linhas.push(["Inf. complementares", pedido.informacoes_complementares]);
  $("#revisao-dados").innerHTML = linhas.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("");
  $("#confirmar-emissao").disabled = previa.erros.length > 0;
  $("#revisao").hidden = false;
  $("#revisao").scrollIntoView({ behavior: "smooth" });
});

$("#voltar-edicao").addEventListener("click", () => {
  $("#revisao").hidden = true;
  $("#valor").focus();
});

$("#confirmar-emissao").addEventListener("click", async () => {
  const botao = $("#confirmar-emissao");
  botao.disabled = true;
  botao.textContent = "Enviando à Sefin...";
  const res = $("#resultado");
  try {
    const r = await chamar("emitir", { pedido: estado.pedido });
    $("#revisao").hidden = true;
    if (r.autorizada) {
      res.innerHTML = `<div class="aviso ok"><strong>NFS-e ${esc(r.numero ?? "")} autorizada.</strong><br>
          Chave: <code>${esc(r.chave)}</code></div>
        ${r.avisos.length ? `<div class="aviso atencao"><ul>${r.avisos.map((a) => `<li>${esc(a)}</li>`).join("")}</ul></div>` : ""}
        <div class="acoes" style="justify-content:flex-start">
          <button class="botao primario" data-abrir="pdf">Abrir PDF</button>
          <button class="botao" data-abrir="pasta">Abrir pasta</button>
        </div>`;
      res.querySelector('[data-abrir="pdf"]').onclick = () => abrirNota(r.nfse_id, false);
      res.querySelector('[data-abrir="pasta"]').onclick = () => abrirNota(r.nfse_id, true);
      // Prepara a próxima nota (mesmo serviço e mês) sem permitir reenviar a anterior por engano.
      escolherTomador(null);
      $("#info-compl").value = "";
      aplicarServico();
      toast(`NFS-e ${r.numero ?? ""} autorizada.`);
    } else {
      res.innerHTML = `<div class="aviso atencao"><strong>Sem resposta definitiva da Sefin.</strong><br>${esc(r.motivo)}
        <br>Confira em "Notas emitidas" (botão Verificar) antes de emitir de novo.</div>`;
    }
  } catch (e) {
    res.innerHTML = `<div class="aviso erro"><strong>Nota não emitida.</strong><br>${esc(e)}</div>`;
  } finally {
    res.hidden = false;
    botao.disabled = false;
    botao.textContent = "Emitir nota";
  }
});

async function abrirNota(nfseId, pasta) {
  try {
    await invoke("abrir_nota", { nfseId, pasta });
  } catch (e) {
    toast(String(e), true);
  }
}

// ---------------------------------------------------------------- notas emitidas

let notasCarregadas = [];

async function carregarNotas() {
  const corpo = $("#lista-notas");
  if (!estado.empresa) {
    corpo.innerHTML = `<tr><td colspan="7" class="vazio">Nenhuma empresa selecionada.</td></tr>`;
    return;
  }
  notasCarregadas = await invoke("listar_notas", { empresaId: estado.empresa.id, mes: vazio($("#mes-notas").value) });
  if (!notasCarregadas.length) {
    corpo.innerHTML = `<tr><td colspan="7" class="vazio">Nenhuma nota neste período.</td></tr>`;
    return;
  }
  corpo.innerHTML = notasCarregadas
    .map((n, i) => {
      const acoes = [];
      if (n.nfse_id) {
        acoes.push(`<button class="botao pequeno" data-acao="pdf" data-i="${i}">PDF</button>`);
        acoes.push(`<button class="botao pequeno" data-acao="pasta" data-i="${i}">Pasta</button>`);
      }
      if (n.situacao === "autorizada") {
        acoes.push(`<button class="botao pequeno perigo" data-acao="cancelar" data-i="${i}">Cancelar</button>`);
      }
      if (n.situacao === "incerta" || n.situacao === "enviando") {
        acoes.push(`<button class="botao pequeno" data-acao="verificar" data-i="${i}">Verificar</button>`);
      }
      const erro = n.erro && n.situacao !== "autorizada" ? `<div class="ajuda">${esc(n.erro)}</div>` : "";
      return `<tr>
        <td>${esc(n.numero_nfse ?? "—")}</td>
        <td>${esc(n.serie)}/${n.numero_dps}</td>
        <td class="truncar" title="${esc(n.tomador)}">${esc(n.tomador ?? "—")}</td>
        <td class="truncar" title="${esc(n.descricao)}">${esc(n.descricao ?? "")}${erro}</td>
        <td class="num">${n.valor ? "R$ " + moeda(n.valor) : ""}</td>
        <td><span class="selo ${esc(n.situacao)}">${esc(NOME_SITUACAO[n.situacao] ?? n.situacao)}</span></td>
        <td><div class="acoes">${acoes.join("")}</div></td>
      </tr>`;
    })
    .join("");
}

$("#mes-notas").addEventListener("change", () => carregarNotas().catch((e) => toast(String(e), true)));
$("#todas-notas").addEventListener("click", () => {
  $("#mes-notas").value = "";
  carregarNotas().catch((e) => toast(String(e), true));
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

function cancelarNota(n) {
  const dlg = $("#dlg-cancelar");
  $("#cancelar-nota").textContent = `NFS-e ${n.numero_nfse ?? ""} — ${n.tomador ?? "sem tomador"} — R$ ${moeda(n.valor)}`;
  $("form", dlg).reset();
  abrirDialogo(dlg, $("#erro-cancelar"), async (d) => {
    const justificativa = d.justificativa.trim();
    if (justificativa.length < 15) throw "A justificativa precisa ter pelo menos 15 caracteres.";
    await chamar("cancelar", { nfseId: n.nfse_id, motivo: d.motivo, justificativa });
    toast(`NFS-e ${n.numero_nfse ?? ""} cancelada.`);
    await carregarNotas();
  });
}

// ---------------------------------------------------------------- tomadores

let tomadoresCarregados = [];

async function carregarTomadores() {
  const corpo = $("#lista-tomadores");
  if (!estado.empresa) {
    corpo.innerHTML = `<tr><td colspan="5" class="vazio">Nenhuma empresa selecionada.</td></tr>`;
    return;
  }
  tomadoresCarregados = await invoke("buscar_tomadores", {
    empresaId: estado.empresa.id,
    termo: $("#busca-tomadores").value,
  });
  corpo.innerHTML = tomadoresCarregados.length
    ? tomadoresCarregados
        .map(
          (t, i) => `<tr>
          <td style="white-space:nowrap">${esc(formatarDocumento(t.tomador.documento))}</td>
          <td>${esc(t.tomador.nome)}</td>
          <td>${esc(t.tomador.endereco?.municipio ?? "")}</td>
          <td class="truncar">${esc(t.emails ?? "")}</td>
          <td><button class="botao pequeno" data-i="${i}">Editar</button></td>
        </tr>`,
        )
        .join("")
    : `<tr><td colspan="5" class="vazio">Nenhum tomador encontrado (a lista mostra até 20; use a busca).</td></tr>`;
}

let buscaTomadores = 0;
$("#busca-tomadores").addEventListener("input", () => {
  clearTimeout(buscaTomadores);
  buscaTomadores = setTimeout(() => carregarTomadores().catch((e) => toast(String(e), true)), 250);
});

$("#lista-tomadores").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-i]");
  if (b) editarTomador(tomadoresCarregados[Number(b.dataset.i)], () => carregarTomadores());
});

$("#novo-tomador").addEventListener("click", () => editarTomador(null, () => carregarTomadores()));

/** Abre o diálogo de tomador; `depois(tomadorDto)` recebe o tomador gravado. */
function editarTomador(dto, depois) {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  const dlg = $("#dlg-tomador");
  const form = $("form", dlg);
  form.reset();
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
      const faltando = [
        ["CEP", d.cep],
        ["município", d.municipio],
        ["logradouro", d.logradouro],
        ["bairro", d.bairro],
      ]
        .filter(([, v]) => !vazio(v))
        .map(([k]) => k);
      if (faltando.length) throw `Endereço incompleto: falta ${faltando.join(", ")}.`;
      if (soDigitos(d.municipio).length !== 7) throw "Município: informe o código IBGE com 7 dígitos.";
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
    const pessoaId = await chamar("salvar_tomador", { empresaId: estado.empresa.id, tomador, emails });
    toast("Tomador salvo.");
    depois?.({ pessoa_id: pessoaId, tomador, emails });
  });
}

// ---------------------------------------------------------------- serviços

const NOME_RETENCAO = { irrf: "IRRF", csll: "CSLL", cp: "INSS", pis: "PIS", cofins: "COFINS" };

async function carregarServicos() {
  const corpo = $("#lista-servicos");
  if (!estado.empresa) {
    corpo.innerHTML = `<tr><td colspan="6" class="vazio">Nenhuma empresa selecionada.</td></tr>`;
    return;
  }
  estado.servicos = await invoke("listar_servicos", { empresaId: estado.empresa.id });
  renderizarServicosEmitir();
  corpo.innerHTML = estado.servicos.length
    ? estado.servicos
        .map((s, i) => {
          const ret = Object.entries(s.tributacao.retencoes ?? {})
            .map(([k, v]) => `${NOME_RETENCAO[k]} ${percentualTela(v)}%`)
            .join(", ");
          return `<tr>
            <td>${esc(s.nome)}</td>
            <td>${esc(s.codigo_trib_nac)}</td>
            <td>${esc(s.nbs ?? "")}</td>
            <td class="num">${s.valor_sugerido ? "R$ " + moeda(s.valor_sugerido) : ""}</td>
            <td>${esc(ret || "—")}</td>
            <td><div class="acoes">
              <button class="botao pequeno" data-acao="editar" data-i="${i}">Editar</button>
              <button class="botao pequeno perigo" data-acao="desativar" data-i="${i}">Remover</button>
            </div></td>
          </tr>`;
        })
        .join("")
    : `<tr><td colspan="6" class="vazio">Nenhum serviço cadastrado para esta empresa.</td></tr>`;
}

$("#lista-servicos").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const s = estado.servicos[Number(b.dataset.i)];
  if (b.dataset.acao === "editar") return editarServico(s);
  if (!confirm(`Remover o serviço "${s.nome}" da lista? As notas já emitidas não mudam.`)) return;
  try {
    await chamar("desativar_servico", { id: s.id });
    await carregarServicos();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#novo-servico").addEventListener("click", () => editarServico(null));

/** Percentual digitado ("11,48") → texto aceito pelo backend, ou null. */
function percentualDigitado(t) {
  const v = vazio(t);
  return v == null ? null : v.replace(",", ".");
}

function editarServico(s) {
  if (!estado.empresa) return toast("Escolha a empresa.", true);
  const dlg = $("#dlg-servico");
  const form = $("form", dlg);
  form.reset();
  if (s) {
    const t = s.tributacao;
    const pc = t.federais?.pis_cofins ?? {};
    const r = t.retencoes ?? {};
    const tot = t.total_tributos;
    const dados = {
      nome: s.nome,
      codigo_trib_nac: s.codigo_trib_nac,
      nbs: s.nbs,
      descricao: s.descricao,
      codigo_trib_mun: s.codigo_trib_mun,
      valor_sugerido: s.valor_sugerido ? moeda(s.valor_sugerido) : "",
      trib_issqn: t.issqn?.tributacao ?? "tributavel",
      ret_issqn: t.issqn?.retencao ?? "nao_retido",
      aliq_issqn: percentualTela(t.issqn?.aliquota),
      cst: pc.cst ?? "00",
      tp_ret: String(pc.tipo_retencao ?? 0),
      r_irrf: percentualTela(r.irrf),
      r_csll: percentualTela(r.csll),
      r_cp: percentualTela(r.cp),
      r_pis: percentualTela(r.pis),
      r_cofins: percentualTela(r.cofins),
      tot_tipo: "nao_informado",
    };
    if (tot && typeof tot === "object" && "percentual_simples" in tot) {
      dados.tot_tipo = "percentual_simples";
      dados.tot_sn = percentualTela(tot.percentual_simples);
    } else if (tot && typeof tot === "object" && "percentuais" in tot) {
      dados.tot_tipo = "percentuais";
      dados.tot_fed = percentualTela(tot.percentuais.federal);
      dados.tot_est = percentualTela(tot.percentuais.estadual);
      dados.tot_mun = percentualTela(tot.percentuais.municipal);
    }
    preencherForm(form, dados);
  }
  abrirDialogo(dlg, $("#erro-servico"), async (d) => {
    const retencoes = {};
    for (const k of ["irrf", "csll", "cp", "pis", "cofins"]) {
      const v = percentualDigitado(d["r_" + k]);
      if (v != null) retencoes[k] = v;
    }
    let total_tributos = "nao_informado";
    if (d.tot_tipo === "percentual_simples") {
      const p = percentualDigitado(d.tot_sn);
      if (p == null) throw "Informe o percentual do Simples Nacional (ou escolha \"Não informar\").";
      total_tributos = { percentual_simples: p };
    } else if (d.tot_tipo === "percentuais") {
      total_tributos = {
        percentuais: {
          federal: percentualDigitado(d.tot_fed) ?? "0",
          estadual: percentualDigitado(d.tot_est) ?? "0",
          municipal: percentualDigitado(d.tot_mun) ?? "0",
        },
      };
    }
    const issqn = { tributacao: d.trib_issqn, retencao: d.ret_issqn };
    const aliq = percentualDigitado(d.aliq_issqn);
    if (aliq != null) issqn.aliquota = aliq;
    const servico = {
      id: s?.id ?? null,
      empresa_id: estado.empresa.id,
      nome: d.nome.trim(),
      descricao: d.descricao.trim(),
      codigo_trib_nac: d.codigo_trib_nac,
      codigo_trib_mun: vazio(d.codigo_trib_mun),
      nbs: vazio(d.nbs),
      tributacao: {
        ...(s?.tributacao ?? {}), // preserva grupos sem campo na tela (ex.: IBS/CBS)
        issqn,
        federais: {
          ...(s?.tributacao?.federais ?? {}),
          pis_cofins: { cst: d.cst.trim() || "00", tipo_retencao: Number(d.tp_ret) },
        },
        total_tributos,
        retencoes,
      },
      valor_sugerido: valorDigitado(d.valor_sugerido),
    };
    await chamar("salvar_servico", { servico });
    toast("Serviço salvo.");
    await carregarServicos();
  });
}

// ---------------------------------------------------------------- empresas

async function renderizarEmpresas() {
  const corpo = $("#lista-empresas");
  corpo.innerHTML = estado.empresas.length
    ? estado.empresas
        .map((e, i) => {
          const c = e.certificado;
          const cert = c
            ? `<span style="${c.dias_para_vencer <= 30 ? "color:var(--vermelho)" : ""}">até ${esc(c.valido_ate)}</span>
               <div class="ajuda">${esc(c.titular)}</div>`
            : `<span style="color:var(--vermelho)">não importado</span>`;
          return `<tr>
            <td style="white-space:nowrap">${esc(e.cnpj_formatado)}</td>
            <td>${esc(e.nome)}</td>
            <td><span class="selo ${e.ambiente}">${NOME_AMBIENTE[e.ambiente]}</span></td>
            <td>${esc(e.serie)}</td>
            <td>${cert}</td>
            <td><div class="acoes">
              <button class="botao pequeno" data-acao="certificado" data-i="${i}">Certificado</button>
              <button class="botao pequeno" data-acao="numeracao" data-i="${i}">Numeração / IM</button>
            </div></td>
          </tr>`;
        })
        .join("")
    : `<tr><td colspan="6" class="vazio">Nenhuma empresa cadastrada.</td></tr>`;
}

$("#lista-empresas").addEventListener("click", (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const e = estado.empresas[Number(b.dataset.i)];
  if (b.dataset.acao === "certificado") importarCertificado(e);
  else editarNumeracao(e);
});

$("#nova-empresa").addEventListener("click", () => {
  const dlg = $("#dlg-empresa");
  $("form", dlg).reset();
  abrirDialogo(dlg, $("#erro-empresa"), async (d) => {
    if (d.simples === "me_epp" && !d.apuracao) throw "Para ME/EPP optante, escolha a forma de apuração.";
    const regime = { simples: d.simples, regime_especial: Number(d.regime_especial) };
    if (d.simples === "me_epp") regime.apuracao_simples = d.apuracao;
    if (d.ambiente === "producao" && !confirm("Cadastrar em PRODUÇÃO? As notas emitidas terão valor fiscal.")) {
      throw "Cadastro não confirmado.";
    }
    const id = await chamar("cadastrar_empresa", {
      empresa: {
        cnpj: d.cnpj,
        nome: d.nome.trim(),
        inscricao_municipal: vazio(d.im),
        municipio_emissor: soDigitos(d.municipio),
        regime,
        ambiente: d.ambiente,
        serie: d.serie,
        telefone: vazio(d.telefone),
        email: vazio(d.email),
      },
    });
    toast("Empresa cadastrada. Agora importe o certificado digital.");
    estado.empresa = { id };
    await carregarEmpresas();
    await renderizarEmpresas();
    const nova = estado.empresas.find((x) => x.id === id);
    if (nova) setTimeout(() => importarCertificado(nova), 0);
  });
});

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
    await carregarEmpresas();
    await renderizarEmpresas();
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
  abrirDialogo(dlg, $("#erro-numeracao"), async (d) => {
    if (vazio(d.proximo)) {
      const proximo = Number(d.proximo);
      if (!Number.isInteger(proximo) || proximo < 1) throw "Próximo número inválido.";
      if (!confirm(`Próxima DPS da série ${empresa.serie} será a nº ${proximo}. Confirma?`)) throw "Não confirmado.";
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
    await carregarEmpresas();
    await renderizarEmpresas();
  });
}

// ---------------------------------------------------------------- configurações

async function carregarConfiguracoes() {
  $("#pasta-saida").value = await invoke("obter_pasta");
  const venc = await invoke("certificados_vencendo", { dias: 60 });
  $("#lista-vencimentos").innerHTML = venc.length
    ? venc
        .map(
          (v) => `<tr><td>${esc(v.empresa)}</td><td>${esc(v.valido_ate)}</td>
            <td class="num" style="color:var(--vermelho)">${v.dias < 0 ? "vencido" : v.dias + " dia(s)"}</td></tr>`,
        )
        .join("")
    : `<tr><td class="vazio">Nenhum certificado vence nos próximos 60 dias.</td></tr>`;
  await carregarPendentes();
}

async function carregarPendentes() {
  const lista = await invoke("pendentes");
  const nomeEmpresa = (id) => estado.empresas.find((e) => e.id === id)?.nome ?? `empresa ${id}`;
  $("#lista-pendentes").innerHTML = lista.length
    ? lista
        .map(
          (p) => `<tr><td>${esc(nomeEmpresa(p.empresa_id))}</td><td><code>${esc(p.id_dps)}</code>
            <div class="ajuda">${esc(p.erro ?? "")}</div></td>
            <td><button class="botao pequeno" data-dps="${p.dps_id}">Verificar</button></td></tr>`,
        )
        .join("")
    : `<tr><td class="vazio">Nenhum envio pendente.</td></tr>`;
}

$("#lista-pendentes").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-dps]");
  if (!b) return;
  b.disabled = true;
  try {
    toast(await chamar("reconciliar", { dpsId: Number(b.dataset.dps) }));
    await carregarPendentes();
  } catch (e) {
    toast(String(e), true);
    b.disabled = false;
  }
});

$("#salvar-pasta").addEventListener("click", async () => {
  try {
    await chamar("definir_pasta", { pasta: $("#pasta-saida").value });
    toast("Pasta salva.");
  } catch (e) {
    toast(String(e), true);
  }
});

$("#abrir-pasta").addEventListener("click", async () => {
  try {
    await invoke("abrir_arquivo", { caminho: $("#pasta-saida").value });
  } catch (e) {
    toast(String(e), true);
  }
});

// ---------------------------------------------------------------- início

iniciarTrava().catch((e) => mostrarErro($("#erro-trava"), e));
