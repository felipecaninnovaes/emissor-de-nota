# 0004 — Competência igual à data da emissão

Estado: aceita (16/10/2026).

**Contexto.** As notas recorrentes referem-se ao mês anterior, mas o campo `dCompet` da DPS precisa ser coerente com a emissão.

**Decisão.** `dCompet` é a data da emissão. O mês de referência do serviço vai só na descrição, pela variável `{mes_ref}` e afins.

**Consequências.** O mês de referência vale apenas para o texto da nota.

**Atualização (20/10/2026).** O mês de referência sugerido nas telas passou a ser configurável (Configurações → Emissão): mês atual, o padrão, ou mês anterior, para quem cobra o mês que passou. Antes era sempre o mês anterior, o que confundia quem emite pelo mês corrente. A competência continua sendo a data da emissão.
