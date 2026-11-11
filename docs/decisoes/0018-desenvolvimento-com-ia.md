# 0018 — Fluxo de desenvolvimento com IA (spec → plan → implement → review → ship)

Estado: aceita (11/11/2026).

**Contexto.** O projeto é desenvolvido quase todo com Claude Code. As regras viviam só no [AGENTS.md](../../AGENTS.md), como orientação: nada impedia o agente de terminar com testes vermelhos, commitar na `main` ou ler o banco cifrado se a permissão fosse afrouxada. Foi adotado um modelo de fundação para desenvolvimento com IA (hooks, skills, `justfile`, specs), pensado para projetos novos e em parte conflitante com as convenções que o repositório já tinha.

**Decisão.**
- **Contrato de comandos:** o `justfile` (`setup`, `check`, `fmt`, `fmt-check`, `fmt-file`, `lint`, `test`, `build`, `run`, `check-servidor`) é o que agente, hooks e pre-commit chamam. `just check` cobre o workspace como o job Windows do CI; o `servidor/`, que precisa de Postgres, fica em `check-servidor`.
- **Garantias determinísticas:** hooks do Claude Code (`guard-git`, `guard-secrets`, `format-file`, `stop-check`) e pre-commit (`no-commit-to-branch`, gitleaks, `cargo fmt --check`, formato da mensagem). As proteções que já existiam no `.claude/settings.json` (leitura de `emissor.db`, `.cofre`, `.nfbk`, `tmp-não-commitar/`, `servidor/.env`; `--confirmo-producao`; force push e exclusão de tag) foram mantidas.
- **Ciclo** `/spec → /plan → /implement → /review → /ship`, proporcional ao tamanho da mudança, com specs em `docs/specs/` e revisão por subagentes ([workflow.md](../workflow.md)).
- **Git:** branch curta + PR, merge sem squash (rebase), para manter um commit por etapa. Commits em Conventional Commits com título em português.
- **Idioma:** código, comentários, docs e specs continuam em português; o modelo previa código e commits em inglês.
- **Docs do projeto prevalecem:** decisões continuam em `docs/decisoes/` (não `docs/adr/`), arquitetura em [arquitetura.md](../arquitetura.md) e o changelog em `## Não publicado`.

**Por quê.**
- O que precisa acontecer sempre (check verde, não commitar na `main`, não ler dados de clientes) fica garantido por hook, não por instrução.
- Commits Conventional deixam o tipo da mudança explícito e permitem automatizar notas de versão no futuro; o título em português mantém a coerência com o código e o changelog.
- Sem squash, o histórico da `main` continua mostrando cada etapa conferida pelo usuário, como antes.
- Migrar o código para inglês ou renomear `docs/decisoes/` geraria um diff enorme e quebraria links, sem ganho prático.

**Consequências.**
- O hook `Stop` acrescenta o tempo de `just check` ao fim dos turnos com código alterado (só roda quando o estado da árvore mudou desde o último check verde).
- No Windows, `just`, `pre-commit`, `gitleaks` e `jq` são instalados à parte (winget/pip). No Linux, ainda falta adicioná-los ao `devbox.json`, do mesmo commit do nixpkgs dos outros pacotes.
- O CI continua chamando `cargo` direto no job Windows (o Devbox só vale no job Linux); `just check` e o job Windows precisam ficar equivalentes.
- Os 257 commits anteriores não seguem Conventional Commits; a regra vale daqui para a frente.
