// Interface do emissor: liga as telas de index.html aos comandos Tauri (src/comandos/).
// Sem bundler: módulos ES puros em js/, um por tela ou assunto. Argumentos de primeiro nível
// dos comandos vão em camelCase (convenção do Tauri); os campos dentro das structs seguem o
// serde (snake_case).

// Na ordem das antigas seções do app.js; as telas só registram eventos ao carregar.
import { $, invoke, mostrarErro } from "./js/util.js";
import "./js/tabela.js";
import "./js/formularios.js";
import { iniciarTrava } from "./js/trava.js";
import "./js/navegacao.js";
import "./js/componentes.js";
import "./js/emitir.js";
import "./js/notas.js";
import "./js/recorrentes.js";
import "./js/lote.js";
import "./js/planilha.js";
import "./js/tomadores.js";
import "./js/consultas.js";
import "./js/servicos.js";
import "./js/empresas.js";
import "./js/configuracoes.js";
import { carregarAparencia } from "./js/preferencias.js";
import "./js/atualizacao.js";

invoke("versao_app")
  .then((v) => {
    $("#versao-app").textContent = v;
    $("#codigo-fonte-versao").textContent = `https://github.com/felipecaninnovaes/emissor-de-nota/tree/v${v}`;
  })
  .catch(() => {});

carregarAparencia();
iniciarTrava().catch((e) => mostrarErro($("#erro-trava"), e));
