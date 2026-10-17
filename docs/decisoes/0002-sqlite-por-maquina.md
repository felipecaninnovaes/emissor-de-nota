# 0002 — Um banco SQLite por máquina

Estado: aceita (16/10/2026).

**Contexto.** Cada máquina é de um operador e algumas empresas não podem ser vistas por outros usuários. O volume é pequeno e não há servidor de banco no fluxo.

**Decisão.** Um SQLite local por máquina (`emissor.db` em `app_data_dir()`). PostgreSQL e uso em rede ficam fora do escopo.

**Consequências.** Não colocar o arquivo em pasta de rede compartilhada. Sem login nem perfis: uma senha mestra local abre o app e protege os certificados ([ADR 0009](0009-certificado-a1-e-senha-mestra.md)). O backup cifrado fica em Configurações ([arquitetura.md](../arquitetura.md#backup-cifrado)).
