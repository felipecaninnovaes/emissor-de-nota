// Configurações: pasta de saída, backup, e-mail e DPS pendentes.

import { $, chamar, esc, estado, invoke, toast, vazio } from "./util.js";
import { lerForm, limparErro, limparErros, mostrarErros, preencherForm, validarCampos } from "./formularios.js";
import { aparencia } from "./preferencias.js";

export async function carregarConfiguracoes() {
  $("#ap-zoom").value = aparencia.zoom;
  $("#ap-densidade").value = aparencia.densidade;
  $("#pasta-saida").value = await invoke("obter_pasta");
  await carregarBackup();
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

$("#abrir-pasta").addEventListener("click", async () => {
  try {
    await invoke("abrir_arquivo", { caminho: $("#pasta-saida").value });
  } catch (e) {
    toast(String(e), true);
  }
});
