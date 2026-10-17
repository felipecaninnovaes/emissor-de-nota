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
  $("#mes-lote").value = mesAnterior();
  $("#mes-planilha").value = mesAnterior();
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
    lote: carregarLote,
    recorrentes: carregarRecorrencias,
    planilha: async () => desenharPlanilha(),
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

/**
 * Busca de tomador com lista de sugestões (setas, Enter, Esc, clique). Usada na emissão e no
 * diálogo de recorrência; `aoEscolher(tomadorDto)` recebe o escolhido.
 */
function ligarBuscaTomador(input, caixa, aoEscolher) {
  let pendente = 0;
  let resultados = [];
  let foco = -1;

  function desenhar() {
    caixa.hidden = false;
    caixa.innerHTML = resultados.length
      ? resultados
          .map(
            (t, i) => `<div data-i="${i}" class="${i === foco ? "foco" : ""}">${esc(t.tomador.nome)}
              <small>${esc(formatarDocumento(t.tomador.documento))}</small></div>`,
          )
          .join("")
      : `<div class="vazio">Nenhum tomador encontrado.</div>`;
  }

  input.addEventListener("input", () => {
    clearTimeout(pendente);
    const termo = input.value.trim();
    if (!termo || !estado.empresa) {
      caixa.hidden = true;
      return;
    }
    pendente = setTimeout(async () => {
      try {
        resultados = await invoke("buscar_tomadores", { empresaId: estado.empresa.id, termo });
      } catch (e) {
        return toast(String(e), true);
      }
      foco = resultados.length ? 0 : -1;
      desenhar();
    }, 200);
  });

  input.addEventListener("keydown", (ev) => {
    if (caixa.hidden || !resultados.length) return;
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      foco = (foco + (ev.key === "ArrowDown" ? 1 : -1) + resultados.length) % resultados.length;
      desenhar();
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      if (foco >= 0) aoEscolher(resultados[foco]);
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      caixa.hidden = true;
    }
  });

  caixa.addEventListener("mousedown", (ev) => {
    const item = ev.target.closest("[data-i]");
    if (item) aoEscolher(resultados[Number(item.dataset.i)]);
  });

  input.addEventListener("blur", () => setTimeout(() => (caixa.hidden = true), 150));
}

ligarBuscaTomador($("#termo-tomador"), $("#resultados-tomador"), (t) => escolherTomador(t));

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
        ${r.avisos
          .map((a) => `<div class="aviso ${a.startsWith("E-mail enviado") ? "ok" : "atencao"}">${esc(a)}</div>`)
          .join("")}
        <div class="acoes" style="justify-content:flex-start">
          <button class="botao primario" data-abrir="pdf">Abrir PDF</button>
          <button class="botao" data-abrir="pasta">Abrir pasta</button>
          <button class="botao" data-recorrente title="Cria uma recorrência com este tomador, serviço, valor e descrição">Tornar recorrente</button>
        </div>`;
      res.querySelector('[data-abrir="pdf"]').onclick = () => abrirNota(r.nfse_id, false);
      res.querySelector('[data-abrir="pasta"]').onclick = () => abrirNota(r.nfse_id, true);
      const emitido = estado.pedido;
      const tomadorEmitido = estado.tomador;
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
        });
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
      if (n.nfse_id && (n.situacao === "autorizada" || n.situacao === "cancelada")) {
        acoes.push(`<button class="botao pequeno" data-acao="email" data-i="${i}">E-mail</button>`);
      }
      if (n.situacao === "autorizada") {
        acoes.push(`<button class="botao pequeno perigo" data-acao="cancelar" data-i="${i}">Cancelar</button>`);
      }
      if (n.situacao === "rejeitada") {
        acoes.push(`<button class="botao pequeno" data-acao="refazer" data-i="${i}">Corrigir e emitir</button>`);
      }
      if (n.situacao === "incerta" || n.situacao === "enviando") {
        acoes.push(`<button class="botao pequeno" data-acao="verificar" data-i="${i}">Verificar</button>`);
      }
      const erro = n.erro && n.situacao !== "autorizada" ? `<div class="ajuda">${esc(n.erro)}</div>` : "";
      return `<tr>
        <td class="dado">${esc(n.numero_nfse ?? "—")}</td>
        <td class="dado">${esc(n.serie)}/${n.numero_dps}</td>
        <td class="truncar" title="${esc(n.tomador)}">${esc(n.tomador ?? "—")}</td>
        <td class="truncar" title="${esc(n.descricao)}">${esc(n.descricao ?? "")}${erro}</td>
        <td class="num">${n.valor ? "R$ " + moeda(n.valor) : ""}</td>
        <td><span class="selo ${esc(n.situacao)}">${esc(NOME_SITUACAO[n.situacao] ?? n.situacao)}</span>${
          n.email_enviado_em ? `<div class="ajuda">E-mail enviado ${esc(dataHora(n.email_enviado_em))}</div>` : ""
        }</td>
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
    case "email":
      return enviarEmailNota(n);
    case "refazer":
      return refazerNota(n);
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

/** Leva os dados de uma nota rejeitada para a tela de emissão (sai com o próximo número). */
async function refazerNota(n) {
  let r;
  try {
    r = await chamar("dados_para_refazer", { dpsId: n.dps_id });
  } catch (e) {
    return toast(String(e), true);
  }
  irPara("emitir");
  limparEmissao();
  escolherTomador(r.tomador);
  if (r.servico_id && estado.servicos.some((s) => s.id === r.servico_id)) {
    $("#servico").value = String(r.servico_id);
  }
  $("#valor").value = moeda(r.valor);
  $("#descricao").value = r.descricao; // já com o mês de referência aplicado
  $("#info-compl").value = r.informacoes_complementares ?? "";
  const avisos = [`Refazendo a DPS ${n.serie}/${n.numero_dps}, rejeitada: ${n.erro ?? ""}`];
  if (!r.servico_id) avisos.push("Nenhum serviço do cadastro tem o mesmo código: escolha o serviço.");
  if (n.tomador && !r.tomador) avisos.push(`O tomador "${n.tomador}" não foi encontrado no cadastro desta empresa.`);
  const res = $("#resultado");
  res.innerHTML = `<div class="aviso atencao">${avisos.map(esc).join("<br>")}
    <br>Corrija o que causou a rejeição e clique em Revisar. A nota sai com o próximo número da série.</div>`;
  res.hidden = false;
}

/** "2026-10-16T22:07:26Z" → "16/10/2026 19:07" (horário local). */
function dataHora(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const dois = (x) => String(x).padStart(2, "0");
  return `${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()} ${dois(d.getHours())}:${dois(d.getMinutes())}`;
}

async function enviarEmailNota(n) {
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
    await carregarNotas();
  });
}

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

// ---------------------------------------------------------------- recorrentes

let recorrenciasCarregadas = [];

async function carregarRecorrencias() {
  const corpo = $("#lista-recorrencias");
  if (!estado.empresa) {
    corpo.innerHTML = `<tr><td colspan="5" class="vazio">Nenhuma empresa selecionada.</td></tr>`;
    return;
  }
  recorrenciasCarregadas = await invoke("listar_recorrencias", { empresaId: estado.empresa.id, mes: mesAnterior() });
  corpo.innerHTML = recorrenciasCarregadas.length
    ? recorrenciasCarregadas
        .map(
          (r, i) => `<tr>
          <td>${esc(r.tomador ?? "—")}</td>
          <td>${esc(r.servico)}</td>
          <td class="truncar" title="${esc(r.descricao_no_mes)}">${esc(r.descricao ?? "(a do serviço)")}</td>
          <td class="num">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}${r.valor ? "" : '<div class="ajuda">do serviço</div>'}</td>
          <td><div class="acoes">
            <button class="botao pequeno" data-acao="editar" data-i="${i}">Editar</button>
            <button class="botao pequeno perigo" data-acao="remover" data-i="${i}">Remover</button>
          </div></td>
        </tr>`,
        )
        .join("")
    : `<tr><td colspan="5" class="vazio">Nenhuma recorrência nesta empresa. Crie uma aqui ou use "Tornar recorrente" depois de emitir uma nota.</td></tr>`;
}

$("#lista-recorrencias").addEventListener("click", async (ev) => {
  const b = ev.target.closest("button[data-acao]");
  if (!b) return;
  const r = recorrenciasCarregadas[Number(b.dataset.i)];
  if (b.dataset.acao === "editar") return editarRecorrencia(r);
  if (!confirm(`Remover a recorrência de ${r.tomador ?? "nota sem tomador"}? As notas já emitidas não mudam.`)) return;
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
  $("[data-rec-escolhido]", dlgRecorrencia).hidden = !t;
  $("[data-rec-busca]", dlgRecorrencia).hidden = !!t;
  $("[data-rec-termo]", dlgRecorrencia).value = "";
  $("[data-rec-resultados]", dlgRecorrencia).hidden = true;
  if (t) $("[data-rec-tomador]", dlgRecorrencia).textContent = t.nome;
}

ligarBuscaTomador($("[data-rec-termo]", dlgRecorrencia), $("[data-rec-resultados]", dlgRecorrencia), (t) =>
  escolherTomadorRecorrencia({
    pessoa_id: t.pessoa_id,
    nome: `${t.tomador.nome} — ${formatarDocumento(t.tomador.documento)}`,
  }),
);
$("[data-rec-trocar]", dlgRecorrencia).addEventListener("click", () => escolherTomadorRecorrencia(null));

/** Abre o diálogo de recorrência: `r` é uma existente; `base`, dados para preencher uma nova. */
function editarRecorrencia(r, base = null) {
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
  const dicas = () => {
    const s = estado.servicos.find((x) => x.id === Number(form.elements.servico.value));
    form.elements.valor.placeholder = s?.valor_sugerido ? `${moeda(s.valor_sugerido)} (do serviço)` : "obrigatório";
    form.elements.descricao.placeholder = s?.descricao ?? "";
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
      },
    });
    toast("Recorrência salva.");
    if (!$('section[data-tela="recorrentes"]').hidden) await carregarRecorrencias();
  });
}

// ---------------------------------------------------------------- lote do mês

let loteCarregado = [];
let loteEmAndamento = false;

const podeEmitirNoLote = (r) => !r.erros.length && (!r.no_mes || r.no_mes.libera);
const ambienteDe = (empresaId) => estado.empresas.find((e) => e.id === empresaId)?.ambiente;
const marcadosLote = () =>
  $$("#lista-lote input[type=checkbox]:checked").map((c) => loteCarregado[Number(c.dataset.i)]);
const esperar = (ms) => new Promise((r) => setTimeout(r, ms));

async function carregarLote() {
  const mes = $("#mes-lote").value;
  if (!mes) {
    $("#lista-lote").innerHTML = `<tr><td colspan="6" class="vazio">Escolha o mês de referência.</td></tr>`;
    return;
  }
  const soEmpresa = $("#lote-so-empresa").checked && estado.empresa;
  loteCarregado = await invoke("listar_recorrencias", { empresaId: soEmpresa ? estado.empresa.id : null, mes });
  desenharLote();
}

function situacaoLote(r) {
  const t = r.no_mes;
  if (t && !t.libera) {
    const nota = t.numero_nfse ? ` nº ${esc(t.numero_nfse)}` : "";
    return `<span class="selo ${esc(t.situacao)}">${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)}${nota}</span>`;
  }
  const anterior = t
    ? `<div class="ajuda">Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</div>`
    : "";
  if (r.erros.length) {
    return `<span class="selo rejeitada">Não pode emitir</span>
      <div class="ajuda" style="color:var(--vermelho)">${r.erros.map(esc).join("<br>")}</div>${anterior}`;
  }
  return `<span class="selo pendente">Pendente</span>${anterior}`;
}

function desenharLote() {
  const corpo = $("#lista-lote");
  $("#lote-todos").checked = false;
  if (!loteCarregado.length) {
    corpo.innerHTML = `<tr><td colspan="6" class="vazio">Nenhuma recorrência cadastrada${
      $("#lote-so-empresa").checked ? " nesta empresa" : ""
    }. Cadastre em "Recorrentes".</td></tr>`;
    atualizarBotaoLote();
    return;
  }
  corpo.innerHTML = loteCarregado
    .map(
      (r, i) => `<tr data-i="${i}">
        <td><input type="checkbox" data-i="${i}" ${podeEmitirNoLote(r) ? "" : "disabled"}></td>
        <td>${esc(r.empresa)}${
          ambienteDe(r.empresa_id) === "homologacao" ? ' <span class="selo homologacao">Homologação</span>' : ""
        }</td>
        <td>${esc(r.tomador ?? "—")}</td>
        <td class="truncar" title="${esc(r.descricao_no_mes)}">${esc(r.descricao_no_mes)}</td>
        <td class="num">${r.valor_efetivo ? "R$ " + moeda(r.valor_efetivo) : ""}</td>
        <td data-situacao>${situacaoLote(r)}</td>
      </tr>`,
    )
    .join("");
  atualizarBotaoLote();
}

function atualizarBotaoLote() {
  const n = marcadosLote().length;
  const botao = $("#lote-emitir");
  botao.textContent = n ? `Emitir selecionadas (${n})` : "Emitir selecionadas";
  botao.disabled = loteEmAndamento || !n;
  $("#lote-marcar").disabled = loteEmAndamento;
}

function marcarPendentes(marcar) {
  for (const c of $$("#lista-lote input[type=checkbox]")) if (!c.disabled) c.checked = marcar;
  atualizarBotaoLote();
}

$("#lista-lote").addEventListener("change", atualizarBotaoLote);
$("#lote-marcar").addEventListener("click", () => marcarPendentes(true));
$("#lote-todos").addEventListener("change", (ev) => marcarPendentes(ev.target.checked));
$("#mes-lote").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));
$("#lote-so-empresa").addEventListener("change", () => carregarLote().catch((e) => toast(String(e), true)));

/**
 * Emite os itens em sequência, mostrando o andamento em `andamento` e o resultado na célula de
 * cada linha (`celulaDe(item)`). `emitirUm(item)` chama o comando; com limite de requisições da
 * Sefin, espera e tenta mais uma vez. Devolve o resumo em HTML.
 */
async function emitirEmSequencia({ itens, andamento, celulaDe, descrever, emitirUm }) {
  const contagem = { autorizada: 0, falha: 0, incerta: 0 };
  for (const [n, item] of itens.entries()) {
    andamento.innerHTML = `<div class="aviso atencao">Emitindo ${n + 1} de ${itens.length}: ${esc(
      descrever(item),
    )}... Não feche o programa.</div>`;
    const celula = celulaDe(item);
    celula.innerHTML = `<span class="selo enviando">Enviando...</span>`;
    let resultado;
    for (let tentativa = 1; !resultado; tentativa++) {
      try {
        resultado = { ok: true, res: await emitirUm(item) };
      } catch (e) {
        if (tentativa === 1 && /limite|429|too many/i.test(String(e))) {
          celula.innerHTML = `<span class="selo enviando">Aguardando a Sefin...</span>`;
          await esperar(15000);
          continue;
        }
        resultado = { ok: false, erro: String(e) };
      }
    }
    if (resultado.ok && resultado.res.autorizada) {
      contagem.autorizada++;
      celula.innerHTML = `<span class="selo autorizada">Autorizada nº ${esc(resultado.res.numero ?? "")}</span>${resultado.res.avisos
        .map((a) => `<div class="ajuda">${esc(a)}</div>`)
        .join("")}`;
    } else if (resultado.ok) {
      contagem.incerta++;
      celula.innerHTML = `<span class="selo incerta">Sem resposta</span><div class="ajuda">${esc(resultado.res.motivo)}</div>`;
    } else {
      contagem.falha++;
      celula.innerHTML = `<span class="selo rejeitada">Não emitida</span>
        <div class="ajuda" style="color:var(--vermelho)">${esc(resultado.erro)}</div>`;
    }
  }
  toast(`Concluído: ${contagem.autorizada} autorizada(s).`);
  return (
    `<div class="aviso ${contagem.falha || contagem.incerta ? "atencao" : "ok"}">Concluído: ` +
    `${contagem.autorizada} autorizada(s)` +
    (contagem.falha ? `, ${contagem.falha} não emitida(s) — o motivo está em cada linha` : "") +
    (contagem.incerta ? `, ${contagem.incerta} sem resposta — confira em "Notas emitidas" (Verificar)` : "") +
    ".</div>"
  );
}

/** Confirmação antes de emitir em lote: quantidade, total e quantas em Produção. */
function confirmarEmissao(quantidade, total, producao, referencia) {
  return confirm(
    `Emitir ${quantidade} nota(s)${referencia ? ` com referência ${referencia}` : ""}, total R$ ${moeda(
      total.toFixed(2),
    )}?` + (producao ? `\n\n${producao} em PRODUÇÃO (valor fiscal).` : ""),
  );
}

$("#lote-emitir").addEventListener("click", async () => {
  const itens = marcadosLote();
  const mes = $("#mes-lote").value;
  if (!itens.length || !mes) return;
  const [ano, m] = mes.split("-");
  const producao = itens.filter((r) => ambienteDe(r.empresa_id) === "producao").length;
  const total = itens.reduce((soma, r) => soma + Number(r.valor_efetivo ?? 0), 0);
  if (!confirmarEmissao(itens.length, total, producao, `${m}/${ano}`)) return;

  loteEmAndamento = true;
  atualizarBotaoLote();
  for (const c of $$("#lista-lote input[type=checkbox]")) c.disabled = true;
  const resumo = await emitirEmSequencia({
    itens,
    andamento: $("#lote-andamento"),
    celulaDe: (r) => $(`#lista-lote tr[data-i="${loteCarregado.indexOf(r)}"] [data-situacao]`),
    descrever: (r) => `${r.empresa} → ${r.tomador ?? "sem tomador"}`,
    emitirUm: (r) => invoke("emitir_recorrencia", { recorrenciaId: r.id, mes }),
  });
  loteEmAndamento = false;
  // Recarrega para mostrar a situação gravada (as rejeitadas voltam a ficar pendentes).
  await carregarLote().catch((e) => toast(String(e), true));
  $("#lote-andamento").innerHTML = resumo;
});

// ---------------------------------------------------------------- importar planilha

let planilhaLida = [];
let planilhaArquivo = null; // { nome, bytes }
let planilhaEmAndamento = false;

const podeEmitirDaPlanilha = (it) => !it.erros.length && (!it.anterior || it.anterior.libera);
const marcadosPlanilha = () =>
  $$("#lista-planilha input[type=checkbox]:checked").map((c) => planilhaLida[Number(c.dataset.i)]);

$("#modelo-planilha").addEventListener("click", async () => {
  try {
    toast(`Modelo salvo em ${await chamar("salvar_modelo_planilha")}`);
  } catch (e) {
    toast(String(e), true);
  }
});

$("#arquivo-planilha").addEventListener("change", async (ev) => {
  const arquivo = ev.target.files[0];
  if (!arquivo) return;
  planilhaArquivo = { nome: arquivo.name, bytes: Array.from(new Uint8Array(await arquivo.arrayBuffer())) };
  await lerPlanilha();
});

$("#mes-planilha").addEventListener("change", () => planilhaArquivo && lerPlanilha());
$("#reler-planilha").addEventListener("click", () => planilhaArquivo && lerPlanilha());

async function lerPlanilha() {
  $("#planilha-andamento").innerHTML = "";
  try {
    planilhaLida = await chamar("ler_planilha", {
      nome: planilhaArquivo.nome,
      bytes: planilhaArquivo.bytes,
      empresaId: estado.empresa?.id ?? null,
      mes: $("#mes-planilha").value,
    });
  } catch (e) {
    planilhaLida = [];
    $("#planilha-andamento").innerHTML = `<div class="aviso erro">${esc(e)}</div>`;
  }
  desenharPlanilha();
}

function situacaoPlanilha(it) {
  const t = it.anterior;
  if (t && !t.libera) {
    const nota = t.numero_nfse ? ` nº ${esc(t.numero_nfse)}` : "";
    return `<span class="selo ${esc(t.situacao)}">Já ${esc((NOME_SITUACAO[t.situacao] ?? t.situacao).toLowerCase())}${nota}</span>`;
  }
  if (it.erros.length) {
    const cadastrar =
      !it.pessoa_id && it.documento_tomador && it.empresa_id
        ? `<div style="margin-top:4px"><button class="botao pequeno" data-cadastrar-tomador="${planilhaLida.indexOf(it)}">Cadastrar tomador</button></div>`
        : "";
    return `<span class="selo rejeitada">Não pode emitir</span>
      <div class="ajuda" style="color:var(--vermelho)">${it.erros.map(esc).join("<br>")}</div>${cadastrar}`;
  }
  const aviso = it.vincular_tomador ? `<div class="ajuda">O tomador será vinculado a esta empresa.</div>` : "";
  const antes = t
    ? `<div class="ajuda">Antes: ${esc(NOME_SITUACAO[t.situacao] ?? t.situacao)} — ${esc(t.erro ?? "")}</div>`
    : "";
  return `<span class="selo pendente">Pronta</span>${aviso}${antes}`;
}

function desenharPlanilha() {
  const corpo = $("#lista-planilha");
  $("#planilha-todos").checked = false;
  if (!planilhaLida.length) {
    corpo.innerHTML = `<tr><td colspan="7" class="vazio">${
      planilhaArquivo ? "Nenhuma linha com dados na planilha." : "Escolha a planilha (.xlsx, .xls, .ods ou .csv)."
    }</td></tr>`;
    $("#planilha-resumo").textContent = "";
    atualizarBotaoPlanilha();
    return;
  }
  corpo.innerHTML = planilhaLida
    .map(
      (it, i) => `<tr data-i="${i}">
        <td><input type="checkbox" data-i="${i}" ${podeEmitirDaPlanilha(it) ? "" : "disabled"}></td>
        <td class="num">${it.linha}</td>
        <td>${esc(it.empresa ?? "—")}</td>
        <td>${esc(it.tomador ?? it.documento_tomador)}</td>
        <td class="truncar" title="${esc(it.descricao)}">${esc(it.descricao)}</td>
        <td class="num">${it.valor ? "R$ " + moeda(it.valor) : ""}</td>
        <td data-situacao>${situacaoPlanilha(it)}</td>
      </tr>`,
    )
    .join("");
  const prontas = planilhaLida.filter(podeEmitirDaPlanilha);
  const total = prontas.reduce((soma, it) => soma + Number(it.valor ?? 0), 0);
  const jaEmitidas = planilhaLida.filter((it) => it.anterior && !it.anterior.libera).length;
  const comErro = planilhaLida.length - prontas.length - jaEmitidas;
  $("#planilha-resumo").textContent =
    `${planilhaLida.length} linha(s): ${prontas.length} pronta(s) para emitir (R$ ${moeda(total.toFixed(2))})` +
    (jaEmitidas ? `, ${jaEmitidas} já emitida(s)` : "") +
    (comErro ? `, ${comErro} com erro` : "") +
    ".";
  atualizarBotaoPlanilha();
}

function atualizarBotaoPlanilha() {
  const n = marcadosPlanilha().length;
  const botao = $("#planilha-emitir");
  botao.textContent = n ? `Emitir selecionadas (${n})` : "Emitir selecionadas";
  botao.disabled = planilhaEmAndamento || !n;
}

function marcarProntas(marcar) {
  for (const c of $$("#lista-planilha input[type=checkbox]")) if (!c.disabled) c.checked = marcar;
  atualizarBotaoPlanilha();
}

$("#lista-planilha").addEventListener("change", atualizarBotaoPlanilha);
$("#lista-planilha").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-cadastrar-tomador]");
  if (!b) return;
  const it = planilhaLida[Number(b.dataset.cadastrarTomador)];
  editarTomador(null, () => lerPlanilha(), { empresaId: it.empresa_id, documento: it.documento_tomador });
});
$("#planilha-todos").addEventListener("change", (ev) => marcarProntas(ev.target.checked));
$("#planilha-marcar").addEventListener("click", () => marcarProntas(true));

$("#planilha-emitir").addEventListener("click", async () => {
  const itens = marcadosPlanilha();
  if (!itens.length) return;
  const producao = itens.filter((it) => ambienteDe(it.empresa_id) === "producao").length;
  const total = itens.reduce((soma, it) => soma + Number(it.valor ?? 0), 0);
  if (!confirmarEmissao(itens.length, total, producao, null)) return;

  planilhaEmAndamento = true;
  atualizarBotaoPlanilha();
  for (const c of $$("#lista-planilha input[type=checkbox]")) c.disabled = true;
  const resumo = await emitirEmSequencia({
    itens,
    andamento: $("#planilha-andamento"),
    celulaDe: (it) => $(`#lista-planilha tr[data-i="${planilhaLida.indexOf(it)}"] [data-situacao]`),
    descrever: (it) => `linha ${it.linha}: ${it.tomador ?? it.documento_tomador}`,
    emitirUm: (it) => invoke("emitir_planilha", { item: it, arquivo: planilhaArquivo?.nome ?? null }),
  });
  planilhaEmAndamento = false;
  await lerPlanilha();
  $("#planilha-andamento").innerHTML = resumo;
});

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
          <td class="dado" style="white-space:nowrap">${esc(formatarDocumento(t.tomador.documento))}</td>
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
function editarTomador(dto, depois, { empresaId = estado.empresa?.id, documento = null } = {}) {
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
    const pessoaId = await chamar("salvar_tomador", { empresaId, tomador, emails });
    toast("Tomador salvo.");
    depois?.({ pessoa_id: pessoaId, tomador, emails });
  });
  if (!dto && documento) {
    // Vindo da planilha: já preenche o documento e, se for CNPJ, consulta os dados.
    const cpf = documento.length === 11;
    form.elements.tipo.value = cpf ? "cpf" : "cnpj";
    form.elements.documento.value = formatarDocumento(cpf ? { cpf: documento } : { cnpj: documento });
    form.elements.documento.dispatchEvent(new Event("change"));
  }
}

// ---------------------------------------------------------------- consulta de CNPJ

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
            <td class="dado" style="white-space:nowrap">${esc(e.cnpj_formatado)}</td>
            <td>${esc(e.nome)}</td>
            <td><span class="selo ${e.ambiente}">${NOME_AMBIENTE[e.ambiente]}</span></td>
            <td class="dado">${esc(e.serie)}</td>
            <td>${cert}</td>
            <td><div class="acoes">
              <button class="botao pequeno" data-acao="editar" data-i="${i}">Editar</button>
              <button class="botao pequeno" data-acao="certificado" data-i="${i}">Certificado</button>
              <button class="botao pequeno" data-acao="numeracao" data-i="${i}">Numeração / IM</button>
              <button class="botao pequeno perigo" data-acao="excluir" data-i="${i}">Excluir</button>
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
  else if (b.dataset.acao === "excluir") excluirEmpresa(e);
  else if (b.dataset.acao === "editar") editarEmpresa(e);
  else editarNumeracao(e);
});

async function excluirEmpresa(empresa) {
  const pergunta =
    `Excluir ${empresa.nome} (${empresa.cnpj_formatado})?\n\n` +
    "Sem notas emitidas, o cadastro, o certificado e os serviços são apagados.\n" +
    "Com notas emitidas, a empresa só é arquivada e as notas continuam guardadas.";
  if (!confirm(pergunta)) return;
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
    if (d.simples === "me_epp" && !d.apuracao) throw "Para ME/EPP optante, escolha a forma de apuração.";
    const regime = { simples: d.simples, regime_especial: Number(d.regime_especial) };
    if (d.simples === "me_epp") regime.apuracao_simples = d.apuracao;
    const dados = {
      cnpj: empresa ? empresa.cnpj : d.cnpj,
      nome: d.nome.trim(),
      inscricao_municipal: vazio(d.im),
      municipio_emissor: soDigitos(d.municipio),
      regime,
      ambiente: d.ambiente,
      serie: d.serie,
      telefone: vazio(d.telefone),
      email: vazio(d.email),
    };
    const vaiParaProducao = d.ambiente === "producao" && empresa?.ambiente !== "producao";
    const mudaEmissao = empresa && (d.ambiente !== empresa.ambiente || d.serie.trim() !== empresa.serie);
    if (vaiParaProducao && !confirm(`${empresa ? "Passar" : "Cadastrar"} ${d.nome.trim()} em PRODUÇÃO? As notas emitidas terão valor fiscal.`)) {
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
    await carregarEmpresas();
    await renderizarEmpresas();
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
  await carregarEmail();
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

async function carregarEmail() {
  const { config, tem_senha } = await invoke("obter_email");
  const form = $("#form-email");
  preencherForm(form, { ...config, senha: "", usuario: config.usuario ?? "", nome_remetente: config.nome_remetente ?? "" });
  $("[data-senha-guardada]", form).hidden = !tem_senha;
}

$("#form-email").addEventListener("submit", (ev) => ev.preventDefault());

$("#form-email").elements.seguranca.addEventListener("change", (ev) => {
  const porta = $("#form-email").elements.porta;
  porta.value = { start_tls: 587, ssl: 465, nenhuma: 25 }[ev.target.value] ?? porta.value;
});

function lerConfigEmail() {
  const d = lerForm($("#form-email"));
  return {
    config: {
      servidor: d.servidor.trim(),
      porta: Number(d.porta),
      seguranca: d.seguranca,
      usuario: vazio(d.usuario),
      remetente: d.remetente.trim(),
      nome_remetente: vazio(d.nome_remetente),
      assunto: d.assunto,
      corpo: d.corpo,
      enviar_ao_emitir: d.enviar_ao_emitir,
    },
    senha: d.senha || null,
  };
}

$("#salvar-email").addEventListener("click", async () => {
  try {
    await chamar("salvar_email", lerConfigEmail());
    toast("Configuração de e-mail salva.");
    await carregarEmail();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#testar-email").addEventListener("click", async () => {
  const para = $("#email-teste").value.trim();
  if (!para) return toast("Informe para quem enviar o teste.", true);
  const botao = $("#testar-email");
  botao.disabled = true;
  botao.textContent = "Enviando...";
  try {
    // Salva antes, para testar exatamente o que está na tela.
    await chamar("salvar_email", lerConfigEmail());
    await chamar("testar_email", { para });
    toast(`Teste enviado para ${para}. Confira a caixa de entrada (e o spam).`);
    await carregarEmail();
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
    botao.textContent = "Enviar teste";
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
