# Fluxo de trabalho com IA

Fluxo **spec-driven**: cada fase produz um artefato em Markdown, revisado por um humano, que alimenta a próxima. O tamanho da mudança decide quantas fases usar.

```
/spec ──► spec.md ──► /plan ──► plan.md + tasks.md ──► /implement ──► commits ──► /review ──► /ship ──► PR
  ▲ gate: spec aprovada   ▲ gate: plano aprovado        ▲ gate: just check verde     ▲ gate: humano faz merge
```

## Escolhendo o tamanho

| Mudança | Fases | Exemplo |
|---|---|---|
| Trivial | direto (com teste se mudar comportamento) | typo, rename local, bump de dependência |
| Pequena e clara | `/plan` → `/implement` → `/ship` | novo campo opcional, refactor contido |
| Feature / comportamento público / vaga | `/spec` → `/plan` → `/implement` → `/review` → `/ship` | tela nova, campo novo na DPS, mudança de leiaute |
| Bug | teste que reproduz → correção → `/ship` (`/spec` se a causa for de design) | rejeição da Sefin |

## Fases

### 1. `/spec` — o quê e por quê
Entrevista até ser possível escrever critérios de aceite testáveis. Saída: `docs/specs/NNNN-slug/spec.md` com requisitos (R#), critérios Dado/Quando/Então, fora de escopo, invariantes e questões abertas. **Gate:** usuário aprova (`Status: aprovada`).

Depois da spec, **abra uma sessão nova** (`/clear`). A entrevista enche o contexto de alternativas descartadas; a implementação rende mais começando só com a spec.

### 2. `/plan` — como
Explora o código, escolhe a abordagem e quebra em tarefas pequenas: cada uma cabe em um commit, deixa `just check` verde e tem um teste que a prova. Decisões difíceis de reverter viram ADR. **Gate:** usuário aprova.

### 3. `/implement` — execução
Numa branch `feat/NNNN-slug`, para cada tarefa: teste falhando → implementação mínima → `just test` → `just check` → marcar tarefa → usuário confere → commit Conventional. Desvio relevante do plano interrompe e pergunta. **Gate (automático):** o hook `Stop` não deixa o turno terminar com `just check` vermelho.

### 4. `/review` — olhos que não escreveram o código
Dois subagentes em paralelo, sem acesso à conversa: `spec-reviewer` (rastreabilidade R# → código → teste, bugs com cenário) e `security-reviewer`. Achados são consolidados por severidade; estilo é descartado de propósito — um revisor instruído a achar problemas sempre acha algo, e corrigir tudo gera abstrações desnecessárias.

### 5. `/ship` — PR
Verifica, organiza commits, push (com confirmação) e `gh pr create` com o template. **Gate:** CI verde + merge humano (rebase, sem squash).

## Git
- `main` sempre verde e protegida; trabalho em branches curtas (`feat/`, `fix/`, `refactor/`, `docs/`, `chore/`), vida de horas a poucos dias.
- Conventional Commits com título em português (`fix(emissor): ...`), validados pelo hook `commit-msg`. Código e comentários continuam em português ([AGENTS.md](../AGENTS.md)).
- Merge sem squash: cada etapa conferida continua sendo um commit na `main`.
- Rebase na `main` antes do PR se houver conflito; force push só com `--force-with-lease` na própria branch.

## Onde cada regra vive

| Tipo | Mecanismo | Exemplo |
|---|---|---|
| Tem que acontecer **sempre** | hook / deny rule / CI | não ler `.env`, check verde antes de parar, não commitar na main |
| Conhecimento sempre útil | `AGENTS.md` (regras do projeto) + `CLAUDE.md` (fluxo do Claude Code) | comandos, convenções não óbvias, armadilhas |
| Conhecimento de uma área | `.claude/rules/*.md` com `paths:` | regras de workflows do CI |
| Procedimento | `.claude/skills/*` | `/spec`, `/ship` |
| Trabalho delegado e isolado | `.claude/agents/*` | revisores, test-writer |
| Decisão e motivo | `docs/decisoes/` | `synchronous = FULL`, banco cifrado |

Se uma linha do `CLAUDE.md` é ignorada repetidamente, ou ela está perdida num arquivo longo demais, ou é uma regra sem exceção — e então deveria ser um hook.

## Manutenção do contexto
- Corrigiu a IA duas vezes pela mesma coisa? Adicione uma linha em "Armadilhas" no `CLAUDE.md`.
- A cada release, releia o `CLAUDE.md`: remova o que o código já mostra ou que deixou de ser verdade (`/doctor prompt-audit` ajuda).
- Preferências pessoais vão em `CLAUDE.local.md` (ignorado pelo git).
