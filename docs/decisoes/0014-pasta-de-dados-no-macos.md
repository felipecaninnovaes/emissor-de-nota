# 0014 — Pasta de dados no macOS sem o sufixo `.app`

Estado: aceita (17/10/2026).

**Contexto.** O Tauri nomeia a pasta de dados com o `identifier` (`br.emissor-nfse.app`). No macOS o sufixo `.app` faz o Finder tratar a pasta como pacote de aplicativo: ela não abre como pasta e atrapalha o acesso, o backup e a cópia manual do banco (issue #4).

**Decisão.** Manter o `identifier` e, só no macOS, usar a pasta `br.emissor-nfse` (`crates/app/src/pasta_dados.rs`). Na primeira execução, o que existe na pasta antiga é movido para a nova, arquivo a arquivo e sem sobrescrever nada.

**Por quê.** Mudar o `identifier` mudaria o caminho dos dados em todos os sistemas e o `upgradeCode` do MSI (o Tauri o deriva do identificador), de modo que o Windows deixaria de atualizar no lugar. Restringir ao macOS não afeta quem já usa o app nos outros sistemas.

**Detalhes.**
- O arquivo do `window-state` fica na pasta antiga: o plugin o lê e grava em `app_config_dir()`, que no macOS coincide com ela. Resta ali só esse arquivo (tamanho e posição da janela), sem dados fiscais.
- A migração só renomeia arquivos (banco, `.cofre`, `-wal`, `-shm`, aparência, backups); não abre nem reescreve o banco, então não altera `synchronous = FULL` ([ADR 0008](0008-synchronous-full.md)). Se for interrompida, a próxima execução completa o que falta.
- Se o mesmo arquivo existir nas duas pastas, o da pasta nova prevalece e o antigo fica onde está.
- Quem copiou o banco manualmente deve copiar `emissor.db` e `emissor.cofre` juntos, como sempre.
