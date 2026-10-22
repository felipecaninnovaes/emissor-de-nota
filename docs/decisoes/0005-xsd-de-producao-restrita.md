# 0005 — Validar com o XSD de Produção Restrita

Estado: aceita (16/10/2026).

**Contexto.** O pacote de Produção (09/02/2026) escreve o padrão de `TSSerieDPS` com `^...$`, que em XSD são caracteres literais, e recusa qualquer série, inclusive as de notas autorizadas.

**Decisão.** Os testes validam a DPS contra `xsd/v1.01-prodrest-20261020`. O pacote de Produção não foi incluído.

**Consequências.** Se a Sefin publicar um XSD de Produção corrigido, vale trocar. Ver [xsd/README.md](../../xsd/README.md).
