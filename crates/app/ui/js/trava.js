// Trava: senha mestra que abre o cofre (e o banco cifrado).

import { $, chamar, esc, estado, invoke, mesAnterior, mesAtual, mostrarErro, toast } from "./util.js";
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
  $("#mes-notas").value = mesLembrado("mes-notas", mesAtual());
  $("#mes-lote").value = mesLembrado("mes-lote", mesAnterior());
  $("#mes-planilha").value = mesAnterior();
  await carregarEmpresas();
  await avisarVencimentos();
  procurarAtualizacao(true);
  if (!estado.empresas.length) {
    irPara("empresas");
    toast("Cadastre a primeira empresa para começar.");
    return;
  }
  const ultima = lerPreferencia("tela");
  if (ultima && ultima !== "emitir" && TELAS.includes(ultima)) irPara(ultima);
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
