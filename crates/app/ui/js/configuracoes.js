// Configurações: pasta de saída, backup (local e na nuvem), e-mail e DPS pendentes.

import { $, chamar, copiarTexto, esc, estado, invoke, mesPadrao, toast, vazio } from "./util.js";
import { confirmar, lerForm, limparErro, limparErros, mostrarErros, preencherForm, validarCampos } from "./formularios.js";
import { aparencia } from "./preferencias.js";

export async function carregarConfiguracoes() {
  $("#ap-zoom").value = aparencia.zoom;
  $("#ap-densidade").value = aparencia.densidade;
  $("#cfg-mes-padrao").value = estado.mesPadrao;
  $("#pasta-saida").value = await invoke("obter_pasta");
  await carregarBackup();
  await carregarNuvem();
  await carregarLeiautes();
  await carregarDiagnostico();
  await carregarEmail();
  const venc = await invoke("certificados_vencendo", { dias: 60 });
  $("#lista-vencimentos").innerHTML = venc.length
    ? venc
        .map(
          (v) => `<tr><td>${esc(v.empresa)}</td><td>${esc(v.valido_ate)}</td>
            <td class="num alerta">${v.dias < 0 ? "vencido" : v.dias + " dia(s)"}</td></tr>`,
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

$("#cfg-mes-padrao").addEventListener("change", async (ev) => {
  try {
    await chamar("definir_mes_padrao", { mes: ev.target.value });
    estado.mesPadrao = ev.target.value;
    // a nota em revisão continua com o mês dela; as próximas usam o novo padrão
    if ($("#revisao").hidden) $("#mes-ref").value = mesPadrao();
    toast(ev.target.value === "anterior" ? "As notas passam a sugerir o mês anterior." : "As notas passam a sugerir o mês atual.");
  } catch (e) {
    ev.target.value = estado.mesPadrao;
    toast(String(e), true);
  }
});

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

/** Confere os campos do e-mail (e, no teste, o destinatário); devolve false e marca o erro se algo está errado. */
function emailConfigOk(...extras) {
  const form = $("#form-email");
  limparErros(form);
  for (const el of extras) limparErro(el);
  const erros = [...validarCampos(form), ...extras.flatMap((el) => validarCampos(el.parentElement).filter((e) => e.el === el))];
  if (erros.length) mostrarErros(erros);
  return !erros.length;
}

$("#salvar-email").addEventListener("click", async () => {
  if (!emailConfigOk()) return;
  try {
    await chamar("salvar_email", lerConfigEmail());
    toast("Configuração de e-mail salva.");
    await carregarEmail();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#testar-email").addEventListener("click", async () => {
  if (!emailConfigOk($("#email-teste"))) return;
  const para = $("#email-teste").value.trim();
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

// "Escolher pasta…": o seletor só preenche o campo (cancelar não muda nada); salvar valida como antes.
document.querySelectorAll("[data-escolher]").forEach((botao) => {
  botao.addEventListener("click", async () => {
    const campo = $(botao.dataset.escolher);
    try {
      const pasta = await invoke("escolher_pasta", { inicial: campo.value });
      if (pasta) campo.value = pasta;
    } catch (e) {
      toast(String(e), true);
    }
  });
});

$("#salvar-pasta").addEventListener("click", async () => {
  try {
    await chamar("definir_pasta", { pasta: $("#pasta-saida").value });
    toast("Pasta salva.");
  } catch (e) {
    toast(String(e), true);
  }
});

// Leiaute da DPS por ambiente: grava ao trocar o seletor.
async function carregarLeiautes() {
  const l = await invoke("obter_leiautes");
  $("#leiaute-restrita").value = l.restrita;
  $("#leiaute-producao").value = l.producao;
}

for (const campo of document.querySelectorAll("[data-leiaute]")) {
  campo.addEventListener("change", async () => {
    try {
      await chamar("definir_leiaute", { ambiente: campo.dataset.leiaute, leiaute: campo.value });
      toast("Leiaute salvo.");
    } catch (e) {
      toast(String(e), true);
      carregarLeiautes();
    }
  });
}

// Diagnóstico e log.
let textoDiagnostico = "";

async function carregarDiagnostico() {
  const d = await invoke("obter_diagnostico");
  textoDiagnostico = d.texto;
  $("#texto-diagnostico").value = d.texto;
  $("#log-detalhado").checked = d.detalhado;
}

$("#atualizar-diagnostico").addEventListener("click", () => carregarDiagnostico().catch((e) => toast(String(e), true)));

$("#copiar-diagnostico").addEventListener("click", async () => {
  try {
    await carregarDiagnostico();
    await copiarTexto(textoDiagnostico);
    toast("Diagnóstico copiado.");
  } catch (e) {
    toast(String(e), true);
  }
});

$("#abrir-logs").addEventListener("click", async () => {
  try {
    await chamar("abrir_pasta_logs");
  } catch (e) {
    toast(String(e), true);
  }
});

$("#log-detalhado").addEventListener("change", async (ev) => {
  try {
    await chamar("definir_log_detalhado", { ligado: ev.target.checked });
    toast(ev.target.checked ? "Log detalhado ligado." : "Log detalhado desligado.");
  } catch (e) {
    toast(String(e), true);
    ev.target.checked = !ev.target.checked;
  }
});

async function carregarBackup() {
  const b = await invoke("obter_backup");
  $("#pasta-backup").value = b.pasta;
  $("#ultimo-backup").textContent = b.ultimo
    ? `Último backup: ${new Date(b.ultimo).toLocaleString("pt-BR")}.`
    : b.pasta
      ? "Nenhum backup feito ainda."
      : "Defina a pasta para ativar o backup automático.";
}

$("#salvar-pasta-backup").addEventListener("click", async () => {
  try {
    await chamar("definir_pasta_backup", { pasta: $("#pasta-backup").value });
    toast("Pasta do backup salva.");
    if ($("#pasta-backup").value.trim()) $("#aviso-backup")?.remove();
    await carregarBackup();
  } catch (e) {
    toast(String(e), true);
  }
});

$("#fazer-backup").addEventListener("click", async () => {
  const botao = $("#fazer-backup");
  botao.disabled = true;
  try {
    const nome = await chamar("fazer_backup");
    toast(`Backup gravado: ${nome}`);
    await carregarBackup();
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
  }
});

// Arquivo do backup e banco a gerar: o seletor só preenche o campo; quem confere é `restaurar_backup`.
$("#escolher-restaurar-arquivo").addEventListener("click", async () => {
  const campo = $("#restaurar-arquivo");
  try {
    const arquivo = await invoke("escolher_arquivo", {
      titulo: "Escolher backup",
      filtro: "Backup do emissor",
      extensoes: ["nfbk"],
      inicial: campo.value || $("#pasta-backup").value,
    });
    if (arquivo) campo.value = arquivo;
  } catch (e) {
    toast(String(e), true);
  }
});

$("#escolher-restaurar-destino").addEventListener("click", async () => {
  const campo = $("#restaurar-destino");
  try {
    const destino = await invoke("salvar_arquivo", {
      titulo: "Banco a gerar",
      filtro: "Banco do emissor",
      extensoes: ["db"],
      inicial: campo.value || $("#restaurar-arquivo").value || $("#pasta-backup").value,
      nome: "emissor-restaurado.db",
    });
    if (destino) campo.value = destino;
  } catch (e) {
    toast(String(e), true);
  }
});

$("#restaurar-backup").addEventListener("click", async () => {
  const botao = $("#restaurar-backup");
  botao.disabled = true;
  try {
    await chamar("restaurar_backup", {
      arquivo: $("#restaurar-arquivo").value,
      senha: $("#restaurar-senha").value,
      destino: $("#restaurar-destino").value,
    });
    $("#restaurar-senha").value = "";
    toast("Backup aberto e íntegro. O banco foi gerado no destino informado.");
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
  }
});

// Restauração de verdade: o banco do backup entra no lugar do atual (guardado ao lado) e o sistema reinicia.
$("#restaurar-agora").addEventListener("click", async () => {
  const arquivo = $("#restaurar-arquivo");
  const senha = $("#restaurar-senha");
  limparErro(arquivo);
  limparErro(senha);
  const erros = [];
  if (!arquivo.value.trim()) erros.push({ el: arquivo, msg: "Escolha o arquivo de backup (.nfbk)." });
  if (!senha.value) erros.push({ el: senha, msg: "Informe a senha mestra do backup." });
  if (erros.length) return mostrarErros(erros);
  const pergunta =
    "Restaurar este backup no lugar dos dados atuais?\n\n" +
    "O sistema reinicia e passa a usar o banco do backup: notas e cadastros feitos depois dele não estarão no sistema. " +
    "O banco atual fica guardado na pasta de dados do aplicativo.\n\n" +
    "Ao abrir de novo, use a senha mestra do backup e confira a numeração de cada empresa na Sefin antes de emitir.";
  if (!(await confirmar(pergunta, { ok: "Restaurar e reiniciar" }))) return;
  const botao = $("#restaurar-agora");
  botao.disabled = true;
  botao.textContent = "Restaurando...";
  try {
    await chamar("restaurar_e_reiniciar", { arquivo: arquivo.value, senha: senha.value });
  } catch (e) {
    toast(String(e), true);
    botao.disabled = false;
    botao.textContent = "Restaurar backup…";
  }
});

// Backup na nuvem: endereço e token (cifrado no cofre), situação da assinatura e backups guardados no servidor.
let nuvemUltimo = null;

async function carregarNuvem() {
  const n = await invoke("obter_nuvem");
  $("#nuvem-endereco").value = n.endereco;
  $("#nuvem-token").value = "";
  $("[data-token-guardado]").hidden = !n.tem_token;
  nuvemUltimo = n.ultimo;
  mostrarSituacaoNuvem(null);
  if (n.endereco && n.tem_token) {
    // Sem esperar: o servidor pode estar lento ou fora do ar, e a tela não fica presa.
    atualizarNuvem().catch((e) => mostrarSituacaoNuvem(null, String(e)));
  } else {
    $("#lista-nuvem").innerHTML = "";
  }
}

function mostrarSituacaoNuvem(situacao, erro) {
  $("#nuvem-inativa").hidden = !situacao || situacao.ativo;
  const partes = [];
  if (situacao) partes.push(`Cliente: ${situacao.cliente}. Assinatura ${situacao.ativo ? "ativa" : "inativa"}.`);
  partes.push(nuvemUltimo ? `Último envio: ${new Date(nuvemUltimo).toLocaleString("pt-BR")}.` : "Nenhum backup enviado ainda.");
  if (erro) partes.push(erro);
  $("#nuvem-situacao").textContent = partes.join(" ");
}

async function atualizarNuvem() {
  const situacao = await invoke("testar_nuvem");
  mostrarSituacaoNuvem(situacao);
  await listarNuvem();
  return situacao;
}

function tamanhoLegivel(bytes) {
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.ceil(bytes / 1024)} KB`;
}

async function listarNuvem() {
  const lista = await invoke("listar_backups_nuvem");
  $("#lista-nuvem").innerHTML = lista.length
    ? lista
        .map(
          (b) => `<tr><td>${esc(new Date(b.criado_em).toLocaleString("pt-BR"))}</td><td><code>${esc(b.nome)}</code></td>
            <td class="num">${tamanhoLegivel(b.tamanho)}</td>
            <td><button class="botao pequeno perigo" data-restaurar-nuvem="${b.id}">Restaurar…</button></td></tr>`,
        )
        .join("")
    : `<tr><td class="vazio">Nenhum backup no servidor.</td></tr>`;
}

/** Grava endereço e token (token vazio mantém o guardado); erro de validação fica junto do campo. */
async function salvarNuvem() {
  const endereco = $("#nuvem-endereco");
  const token = $("#nuvem-token");
  limparErro(endereco);
  limparErro(token);
  const erros = [];
  if (!endereco.value.trim()) erros.push({ el: endereco, msg: "Informe o endereço do servidor." });
  if (!token.value.trim() && $("[data-token-guardado]").hidden) erros.push({ el: token, msg: "Informe o token." });
  if (erros.length) {
    mostrarErros(erros);
    return false;
  }
  try {
    await chamar("salvar_nuvem", { endereco: endereco.value, token: token.value });
  } catch (e) {
    mostrarErros([{ el: endereco, msg: String(e) }]);
    return false;
  }
  return true;
}

$("#salvar-nuvem").addEventListener("click", async () => {
  if (!(await salvarNuvem())) return;
  toast("Backup na nuvem salvo.");
  await carregarNuvem();
});

$("#testar-nuvem").addEventListener("click", async () => {
  if (!(await salvarNuvem())) return;
  const botao = $("#testar-nuvem");
  botao.disabled = true;
  try {
    await carregarNuvem();
    const situacao = await atualizarNuvem();
    toast(situacao.ativo ? "Conexão com o servidor de backup funcionando." : "Conectado, mas a assinatura está inativa.");
  } catch (e) {
    mostrarSituacaoNuvem(null, String(e));
    toast(String(e), true);
  } finally {
    botao.disabled = false;
  }
});

$("#enviar-nuvem").addEventListener("click", async () => {
  const botao = $("#enviar-nuvem");
  botao.disabled = true;
  botao.textContent = "Enviando...";
  try {
    const nome = await chamar("enviar_backup_nuvem");
    toast(`Backup enviado à nuvem: ${nome}`);
    await carregarNuvem();
  } catch (e) {
    toast(String(e), true);
  } finally {
    botao.disabled = false;
    botao.textContent = "Enviar agora";
  }
});

$("#listar-nuvem").addEventListener("click", async () => {
  try {
    await atualizarNuvem();
  } catch (e) {
    mostrarSituacaoNuvem(null, String(e));
    toast(String(e), true);
  }
});

// Restaurar da nuvem: baixa, confere e troca o banco como "Restaurar backup…" (o sistema reinicia).
$("#lista-nuvem").addEventListener("click", async (ev) => {
  const botao = ev.target.closest("[data-restaurar-nuvem]");
  if (!botao) return;
  const senha = $("#nuvem-senha");
  limparErro(senha);
  if (!senha.value) return mostrarErros([{ el: senha, msg: "Informe a senha mestra do backup." }]);
  const pergunta =
    "Restaurar este backup da nuvem no lugar dos dados atuais?\n\n" +
    "O sistema baixa o backup, reinicia e passa a usar o banco dele: notas e cadastros feitos depois dele não estarão no sistema. " +
    "O banco atual fica guardado na pasta de dados do aplicativo.\n\n" +
    "Ao abrir de novo, use a senha mestra do backup e confira a numeração de cada empresa na Sefin antes de emitir.";
  if (!(await confirmar(pergunta, { ok: "Restaurar e reiniciar" }))) return;
  botao.disabled = true;
  botao.textContent = "Restaurando...";
  try {
    await chamar("restaurar_da_nuvem", { id: Number(botao.dataset.restaurarNuvem), senha: senha.value });
  } catch (e) {
    toast(String(e), true);
    botao.disabled = false;
    botao.textContent = "Restaurar…";
  }
});

$("#abrir-pasta").addEventListener("click", async () => {
  try {
    await invoke("abrir_arquivo", { caminho: $("#pasta-saida").value });
  } catch (e) {
    toast(String(e), true);
  }
});
