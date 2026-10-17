# 0012 — Retenções do serviço em percentual; o tomador decide se retém

Estado: aceita (16/10/2026; tomador que retém em 17/10/2026, migração `0006`).

**Contexto.** As alíquotas dependem do serviço, mas quem decide se retém é o tomador. Antes, o operador duplicava o serviço (com e sem retenção).

**Decisão.** As retenções (IRRF, CSLL, INSS, PIS, COFINS) são percentuais no serviço, e os valores são calculados sobre o valor de cada nota. Cada retenção do serviço vale "sempre" ou "só para tomador que retém"; o vínculo empresa × tomador tem `retem_federais` e `retem_iss`.
