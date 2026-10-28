---
title: Substituição de NFS-e implementada
author: Felipe
date: 2026-10-27
branch: main
cost: { minutes: 10, tokens: 147004 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/comandos/notas.rs
  - crates/app/src/comandos/recorrencias.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/notas.js
  - crates/nfse-db/src/dps.rs
  - crates/nfse-emissor/src/copia.rs
---

## Decided
- `PedidoEmissao.substituir: Option<SubstituirNota>` (nfse_id, motivo, descricao_motivo). `substituicao_do_pedido` (nfse-emissor/src/substituicao.rs) confere empresa, ambiente, situação `autorizada` e substituta pendente, e devolve o `Substituicao` que `emitir` e o `validar_pedido` do app põem em `modelo.substituicao` depois de `montar_dps` (que continua com `None`).
- A original vira `substituida` dentro de `Banco::registrar_autorizacao`, na mesma transação, lendo `modelo_json` da DPS autorizada: vale também para a autorização achada pela reconciliação.
- PDF da original regerado com SUBSTITUÍDA em `Emissor::gravar` (`atualizar_pdf_da_substituida`); falha vira aviso.
- UI: ação "Substituir nota" (menu Mais, só autorizada) reaproveita `levarParaEmissao` e abre o bloco `#bloco-substituicao` no topo da emissão; `limparEmissao` encerra a substituição.

## Tried and ruled out
- Coluna nova em `dps` para a chave substituída: desnecessária; `substituta_em_andamento` usa `json_extract(modelo_json, '$.substituicao.chave_substituida')` (o SQLCipher embutido tem JSON).

## Watch out
- O evento 105102 (cancelamento por substituição) não é consultado nem gravado; a situação vem só da autorização da substituta.
- xMotivo é opcional no XSD (15 a 255); não inventar obrigatoriedade para o motivo 99.
- `trail check` acusa conflito com learnings de migração/coluna em `dps` por palavras soltas no usuario.md; não há migração nem coluna nova.
