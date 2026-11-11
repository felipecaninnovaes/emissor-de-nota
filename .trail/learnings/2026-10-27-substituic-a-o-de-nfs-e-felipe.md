---
title: "Substituição de NFS-e: o que já existe e o que falta"
author: Felipe
date: 2026-10-27
branch: main
cost: { minutes: 1, tokens: 42229 }
touches:
  - docs/fiscal.md
---

## Decided
- Ainda nada implementado; levantamento feito a pedido do usuário (aguardando ok para implementar).
- Já existe: `Substituicao` (nfse-core/src/modelo.rs) validada em `Dps::validar`; `gerar_dps` monta `<subst>/chSubstda` (nfse-xml/src/dps.rs); `SituacaoDps::Substituida` no banco, rótulo na UI (util.js) e marca d'água no PDF.
- Plano: `dados_para_substituir` (só Autorizada) reaproveitando `ler_dados_da_dps` de nfse-emissor/src/copia.rs; levar a substituição pelo `PedidoEmissao` até `montar_dps` (hoje fixa `substituicao: None`); após autorizar, marcar a original `substituida` e regerar o PDF; botão "Substituir" em notas.js no padrão do "Copiar".

## Tried and ruled out
- Nada testado ainda.

## Watch out
- Nenhum código grava `SituacaoDps::Substituida` hoje.
- A Sefin cancela a original pelo evento 105102, que a conferência de cancelamento ainda não consulta (docs/fiscal.md).
- Nota substituta recebe número novo; prazos de substituição variam por município (sem parâmetro no sistema).
- Mudança fiscal: testar `<subst>` no XSD NT 004 e NT 009, atualizar fiscal.md e CHANGELOG; versão minor.
