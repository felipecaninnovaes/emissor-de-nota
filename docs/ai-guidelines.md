# Por que este repositório é assim

Documento para **humanos**: explica os princípios por trás da fundação. O agente não precisa lê-lo no dia a dia (tudo que ele precisa está em `AGENTS.md`, `CLAUDE.md`, skills e hooks). A decisão de adotar este fluxo, e onde ele diverge do modelo original, está em [decisoes/0018](decisoes/0018-desenvolvimento-com-ia.md).

## Princípios

1. **Escreva só o que a IA não consegue descobrir.** Agentes leem código muito bem. O `CLAUDE.md` guarda comandos de verificação, convenções que não aparecem no código, armadilhas e decisões tomadas fora do repositório. Estrutura de pastas e regras de estilo ficam de fora (linters e formatters cuidam do estilo). Arquivos de contexto gerados automaticamente ou que duplicam o que já está no repositório reduziram a taxa de sucesso e aumentaram o custo em estudo da ETH Zurich (2025).
2. **Contexto curto e progressivo.** Cada linha do `CLAUDE.md` custa atenção em toda tarefa. A raiz aponta para `docs/`, `.claude/rules/` (carregadas por caminho) e skills (carregadas sob demanda).
3. **Determinístico para o que não pode falhar.** Instrução em Markdown é orientação; o modelo pode decidir que não se aplica. O que precisa acontecer *sempre* vira deny rule, hook, pre-commit ou CI — e o conjunto é mantido pequeno.
4. **Sempre um check pass/fail.** `just check` é o contrato. O agente fecha o loop sozinho (muda → verifica → corrige) e o hook `Stop` impede encerrar com ele vermelho. Pedir **evidência** (comando + saída), não "está pronto".
5. **Spec antes de código, sessão limpa para implementar.** Spec versionada com requisitos e critérios de aceite testáveis; tarefas pequenas; implementação numa sessão nova, sem o ruído da entrevista.
6. **Revisão por quem não escreveu.** Subagentes revisores não veem a conversa e não são influenciados pela justificativa do autor. Estilo é descartado de propósito.
7. **Menor privilégio e conteúdo externo como dado.** Segredos fora do alcance (deny rules), confirmação para ações irreversíveis ou externas (push, PR, release), e instrução explícita de que issues, páginas web e saídas de ferramentas não são comandos — vetor real de prompt injection em 2026.
8. **Decisões registradas.** As decisões em `docs/decisoes/` evitam que o agente re-sugira alternativas descartadas.

## O que é garantido vs. orientado

| Regra | Garantido por |
|---|---|
| Não ler `.env`, chaves, `~/.ssh`, `~/.aws`, `emissor.db`, `.cofre`, `.nfbk`, `tmp-não-commitar/` | deny rules em `.claude/settings.json` |
| Agente nunca emite em Produção (`--confirmo-producao`) | deny rule + regra **Produção** do AGENTS.md |
| Não gravar segredos nem escrever em `.env` | hook `guard-secrets.sh` + gitleaks no pre-commit |
| Não commitar/pushar na `main`, sem force push, sem `--no-verify` | hook `guard-git.sh` + pre-commit `no-commit-to-branch` (+ ruleset do GitHub, quando configurado) |
| Push, PR, merge, release, `rm -rf`, `reset --hard` pedem confirmação | regras `ask` |
| Código formatado | hook `format-file.sh` + `just fmt-check` no CI |
| Não terminar com check vermelho | hook `stop-check.sh` + CI |
| Conventional Commits (título em português) | pre-commit `commit-msg` |
| Teste para toda mudança, spec antes de feature, idioma, ADRs | orientação (`CLAUDE.md`, skills) + revisão |

## Fontes consultadas (nov/2026)
- Anthropic — [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices), [Hooks](https://code.claude.com/docs/en/hooks), [Skills](https://code.claude.com/docs/en/skills)
- Phil Schmid — [Writing a Good AGENTS.md](https://www.philschmid.de/writing-good-agents) (resume o estudo da ETH Zurich)
- Addy Osmani — [How to write a good spec for AI agents](https://addyosmani.com/blog/good-spec/) · [AGENTS.md: giving agents project context](https://addyosmani.com/agents/15-agents-md/)
- O'Reilly Radar — [How to write a good spec for AI agents](https://www.oreilly.com/radar/?p=18097)
- Verdent — [Spec-driven development](https://www.verdent.ai/ar/guides/coding/spec-driven-development) (fluxo Spec → Plan → Tasks → Implement do GitHub Spec Kit)
- Hackernoon — [The 5xP framework](https://sia.hackernoon.com/the-5xp-framework-that-makes-ai-agents-build-real-software) (raiz curta com links para contexto detalhado)
- JetBrains — [AI Agent Security Best Practices](https://www.jetbrains.com/pages/ai-agents/security/security-best-practices/) · Prisma — [How to use AI safely](https://www.prisma.io/blog/how-to-use-ai-safely-and-responsibly.md) · [AI coding agent security 2026](https://codepick.dev/en/guides/ai-coding-agent-security-2026/)
- Referências a acompanhar: OWASP Secure Coding with AI Cheat Sheet e OWASP Agentic AI Top 10.
