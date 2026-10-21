---
title: vRetCSLL soma PIS+COFINS também quando o tipo 3 vem do cadastro
author: Felipe
date: 2026-10-21
branch: main
cost: { minutes: 2, tokens: 55954 }
touches:
  - crates/nfse-emissor/src/montagem.rs
  - crates/nfse-emissor/src/testes.rs
---

## Decided
`federais_da_nota` (nfse-emissor/src/montagem.rs) soma vPis/vCofins calculados em `vRetCSLL` quando `tipo_retencao` do cadastro marca PIS/COFINS retidos (1,3,4,5,6,7,9) e não há percentual de retenção no serviço. Espelha a nota do Portal Nacional: vPis/vCofins seguem na DPS e vRetCSLL = CSLL + PIS + COFINS.

## Tried and ruled out
O caminho por percentuais (`retencoes.*`) já somava e zera vPis/vCofins; não foi alterado.

## Watch out
No caminho do cadastro a base do IBS/CBS cai por vPis+vCofins (Portal: 700 → 674,45); no caminho por percentuais não. Nada foi validado com emissão real.
