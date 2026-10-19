// Componentes: combobox de busca, escolha de tomador (emissão e recorrência) e campo de arquivo.

import { $, esc, estado, formatarDocumento, invoke, toast } from "./util.js";

/**
 * Alterna entre a busca de tomador e o cartão do escolhido (com "Trocar"). `texto` vazio volta para a
 * busca. Serve à emissão e ao diálogo de recorrência.
 */
export function mostrarTomadorEscolhido({ escolhido, busca, resultados, termo, rotulo }, texto) {
  escolhido.hidden = !texto;
  busca.hidden = !!texto;
  resultados.hidden = true;
  termo.value = "";
  if (texto) rotulo.textContent = texto;
}

/** Vincula à empresa escolhida um tomador cadastrado por outra; e-mails e retenção começam vazios. */
export async function vincularTomador(t) {
  const vinculado = await invoke("vincular_tomador", { empresaId: estado.empresa.id, pessoaId: t.pessoa_id });
  toast("Tomador vinculado a esta empresa. Confira e-mails e retenções em Tomadores.");
  return vinculado;
}

/**
 * Campo de busca no padrão combobox (ARIA 1.2): o foco fica no campo e a opção ativa é indicada
 * por aria-activedescendant; setas, Home/End, Enter, Esc e clique. `buscar(termo)` devolve a lista
 * (ou null para fechar), `opcao(item)` o HTML de cada opção e `aoEscolher(item)` recebe o escolhido.
 */
export function ligarCombobox(input, caixa, { rotulo, vazio, contagem, buscar, opcao, aoEscolher }) {
  let pendente = 0;
  let sequencia = 0;
  let resultados = [];
  let foco = -1;
  caixa.id ||= `${input.id}-lista`;
  caixa.setAttribute("role", "listbox");
  caixa.setAttribute("aria-label", `${rotulo} encontrados`);
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-controls", caixa.id);
  input.setAttribute("aria-expanded", "false");

  // a lista também é escondida por quem escolhe/troca o item: aria-expanded acompanha o atributo hidden
  new MutationObserver(() => {
    input.setAttribute("aria-expanded", String(!caixa.hidden));
    if (caixa.hidden) input.removeAttribute("aria-activedescendant");
  }).observe(caixa, { attributes: true, attributeFilter: ["hidden"] });

  function desenhar() {
    caixa.hidden = false;
    caixa.innerHTML = resultados.length
      ? resultados
          .map(
            (item, i) => `<div role="option" id="${caixa.id}-${i}" data-i="${i}" aria-selected="${i === foco}"
              class="${i === foco ? "foco" : ""}">${opcao(item)}</div>`,
          )
          .join("")
      : `<div class="vazio" role="option" aria-disabled="true" aria-selected="false">${esc(vazio)}</div>`;
    if (foco >= 0) {
      input.setAttribute("aria-activedescendant", `${caixa.id}-${foco}`);
      document.getElementById(`${caixa.id}-${foco}`)?.scrollIntoView({ block: "nearest" });
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  input.addEventListener("input", () => {
    clearTimeout(pendente);
    // resposta de uma busca antiga que chega depois da nova é descartada
    const minha = ++sequencia;
    const termo = input.value.trim();
    if (!termo) {
      caixa.hidden = true;
      return;
    }
    pendente = setTimeout(async () => {
      let achados;
      try {
        achados = await buscar(termo);
      } catch (e) {
        if (minha === sequencia) toast(String(e), true);
        return;
      }
      if (minha !== sequencia) return;
      if (!achados) {
        caixa.hidden = true;
        return;
      }
      resultados = achados;
      foco = resultados.length ? 0 : -1;
      desenhar();
      anunciar(resultados.length ? contagem(resultados.length) : vazio);
    }, 200);
  });

  input.addEventListener("keydown", (ev) => {
    if (!resultados.length) return;
    if (caixa.hidden) {
      // seta para baixo reabre a última lista
      if (ev.key === "ArrowDown" && input.value.trim()) {
        ev.preventDefault();
        desenhar();
      }
      return;
    }
    const destino = { ArrowDown: foco + 1, ArrowUp: foco - 1, Home: 0, End: resultados.length - 1 }[ev.key];
    if (destino !== undefined) {
      ev.preventDefault();
      foco = (destino + resultados.length) % resultados.length;
      desenhar();
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      if (foco >= 0) aoEscolher(resultados[foco]);
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      caixa.hidden = true;
    }
  });

  // mousedown com preventDefault: o clique escolhe sem tirar o foco do campo antes da hora
  caixa.addEventListener("mousedown", (ev) => {
    ev.preventDefault();
    const item = ev.target.closest("[data-i]");
    if (item) aoEscolher(resultados[Number(item.dataset.i)]);
  });

  input.addEventListener("blur", () => {
    caixa.hidden = true;
  });
}

/**
 * Busca de tomador (emissão e diálogo de recorrência), com os de outras empresas no fim.
 * `aoEscolher(tomadorDto)` recebe o escolhido.
 */
export function ligarBuscaTomador(input, caixa, aoEscolher) {
  ligarCombobox(input, caixa, {
    rotulo: "Tomadores",
    vazio: "Nenhum tomador encontrado.",
    contagem: (n) => `${n} tomador(es) encontrado(s).`,
    async buscar(termo) {
      if (!estado.empresa) return null;
      const resultados = await invoke("buscar_tomadores", { empresaId: estado.empresa.id, termo });
      const outros = await invoke("buscar_tomadores_de_outras_empresas", { empresaId: estado.empresa.id, termo });
      return [...resultados, ...outros.map((t) => ({ ...t, de_outra_empresa: true }))];
    },
    opcao: (t) => `${esc(t.tomador.nome)}
              <small>${esc(formatarDocumento(t.tomador.documento))}${t.de_outra_empresa ? " · de outra empresa: vincular" : ""}</small>`,
    // tomador de outra empresa: vincula a esta (sem cadastrar de novo) e só então escolhe
    async aoEscolher(t) {
      if (t.de_outra_empresa) {
        try {
          t = await vincularTomador(t);
        } catch (e) {
          return toast(String(e), true);
        }
      }
      aoEscolher(t);
    },
  });
}

/** Anuncia um texto curto no leitor de tela sem mostrar nada (região viva escondida). */
export function anunciar(texto) {
  const el = $("#anuncio");
  el.textContent = "";
  setTimeout(() => (el.textContent = texto), 50);
}

export const FILTRO_PLANILHA = { filtro: "Planilha", extensoes: ["xlsx", "xlsm", "xls", "ods", "csv"] };

/**
 * Campo de arquivo com o seletor nativo do sistema (no Linux, pelo xdg-desktop-portal; o
 * `<input type="file">` do WebKitGTK abre o diálogo do GTK). O `campo` (somente leitura) mostra o
 * nome; o botão, o clique ou Enter no campo abrem o seletor. `aoEscolher({ nome, bytes })` recebe o
 * conteúdo; com `caminho`, o campo mostra e `aoEscolher` recebe só o caminho (quem usa lê o
 * arquivo). Cancelar não muda nada.
 */
export function ligarEscolhaArquivo(campo, botao, { titulo, filtro, extensoes, caminho = false }, aoEscolher) {
  const escolher = async () => {
    try {
      const arquivo = caminho
        ? await invoke("escolher_arquivo", { titulo, filtro, extensoes, inicial: campo.value })
        : await invoke("ler_arquivo_escolhido", { titulo, filtro, extensoes });
      if (!arquivo) return;
      campo.value = caminho ? arquivo : arquivo.nome;
      campo.dispatchEvent(new Event("change", { bubbles: true }));
      await aoEscolher(arquivo);
    } catch (e) {
      toast(String(e), true);
    }
  };
  botao.addEventListener("click", escolher);
  campo.addEventListener("click", escolher);
  campo.addEventListener("keydown", (ev) => {
    if (ev.key !== "Enter") return;
    ev.preventDefault();
    escolher();
  });
}
