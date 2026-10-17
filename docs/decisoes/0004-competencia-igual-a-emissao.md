# 0004 — Competência igual à data da emissão

Estado: aceita (16/10/2026).

**Contexto.** As notas recorrentes referem-se ao mês anterior, mas o campo `dCompet` da DPS precisa ser coerente com a emissão.

**Decisão.** `dCompet` é a data da emissão. O mês de referência do serviço vai só na descrição, pela variável `{mes_ref}` e afins.

**Consequências.** O mês de referência padrão na tela de emissão é o mês anterior e vale apenas para o texto da nota.
