# 0006 — Numeração e Id da DPS únicos por ambiente

Estado: aceita (16/10/2026, commit `e126819`).

**Contexto.** Produção e Produção Restrita são independentes; o mesmo número de DPS pode existir nos dois.

**Decisão.** A numeração de cada ambiente começa do 1 e o Id da DPS é único por ambiente (`dps_unica_por_ambiente.sql` em `crates/nfse-db/ajustes/`).

**Consequências.** A troca de ambiente de uma empresa é recusada enquanto houver envio sem resposta.
