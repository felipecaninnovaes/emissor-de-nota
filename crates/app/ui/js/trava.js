// Trava: senha mestra que abre o cofre (e o banco cifrado).

import { $, chamar, esc, estado, invoke, mesAtual, mesPadrao, mostrarErro, toast } from "./util.js";
import { carregarEmpresas, irPara, lerPreferencia } from "./navegacao.js";
import { TELAS, mesLembrado } from "./preferencias.js";
import { procurarAtualizacao } from "./atualizacao.js";

let cofreExiste = true;

// Descarta a chave da memória e recarrega a interface: volta à tela de senha, sem dados na tela.
$("#bloquear").addEventListener("click", async () => {
  try {
    await invoke("bloquear_cofre");
    location.reload();
  } catch (e) {
    toast(String(e), true);
  }
});

export async function iniciarTrava() {
  const s = await invoke("situacao_cofre");
  if (s.aberto) return entrar([]);
  cofreExiste = s.existe;
  if (!cofreExiste) {
    $("#trava-ajuda").textContent =
      "Primeiro uso: crie a senha mestra (mínimo 10 caracteres; uma frase é mais fácil de lembrar).";
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
    if ([...senha].length < 10) return mostrarErro(erro, "A senha mestra precisa ter pelo menos 10 caracteres.");
    if (senha !== $("#senha2").value) return mostrarErro(erro, "As senhas não conferem.");
    if (!$("#senha-anotada").checked) {
      return mostrarErro(erro, "Anote a senha mestra em lugar seguro e marque a confirmação: sem ela, nada abre.");
    }
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
  estado.mesPadrao = await invoke("obter_mes_padrao").catch(() => "atual");
  $("#mes-ref").value = mesPadrao();
  $("#mes-notas").value = mesLembrado("mes-notas", mesAtual());
  $("#mes-lote").value = mesLembrado("mes-lote", mesPadrao());
  $("#mes-planilha").value = mesPadrao();
  await carregarEmpresas();
  await avisarVencimentos();
  await avisarBackup();
  procurarAtualizacao(true);
  // Sem empresa, a tela de emissão mostra os primeiros passos, a começar pelo cadastro dela.
  if (!estado.empresas.length) return;
  const ultima = lerPreferencia("tela");
  if (ultima && ultima !== "emitir" && TELAS.includes(ultima)) irPara(ultima);
}

/** Sem pasta de backup, o aviso fica no topo de todas as telas até ela ser definida em Configurações. */
async function avisarBackup() {
  const b = await invoke("obter_backup").catch(() => null);
  if (!b || b.pasta.trim()) return;
  $("#alertas").insertAdjacentHTML(
    "beforeend",
    `<div class="aviso atencao" id="aviso-backup"><span><strong>Backup desligado.</strong> Se este computador
      estragar, as notas, os cadastros e os certificados se perdem. Escolha uma pasta fora dele (pendrive, HD externo ou
      pasta sincronizada com a nuvem).</span>
      <button type="button" class="botao pequeno" data-ir="configuracoes">Configurar backup</button></div>`,
  );
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
