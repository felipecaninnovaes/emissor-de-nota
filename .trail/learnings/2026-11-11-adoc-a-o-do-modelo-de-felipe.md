---
title: Adoção do modelo de fluxo com IA (bootstrap) num repo existente
author: Felipe
date: 2026-11-11
branch: chore/adopt-ai-workflow
cost: { minutes: 4, tokens: 71972 }
touches:
  - CLAUDE.md
  - CLAUDE_old.md
  - docs/adr/0000-template.md
  - docs/adr/0001-record-architecture-decisions.md
  - docs/adr/0002-ai-development-foundation.md
  - docs/adr/README.md
  - docs/ai-guidelines.md
  - docs/architecture.md
  - docs/specs/README.md
  - docs/specs/_template/plan.md
---

## Decided
- Commits passam a Conventional Commits com título em português (`fix(emissor): ...`); código e comentários continuam em pt-BR.
- Trabalho em branches curtas (feat/ fix/ refactor/ docs/ chore/) com PR e merge SEM squash (rebase): cada etapa continua um commit no histórico.
- Docs do projeto prevalecem sobre os do modelo: docs/decisoes/ (não docs/adr/), docs/arquitetura.md (não architecture.md), CHANGELOG `## Não publicado` (não [Unreleased]). ADR de fundação de IA vai para docs/decisoes/0018.
- `just check` = fmt-check + clippy -D warnings + cargo test --workspace (igual ao CI Windows); servidor/ (Postgres) fica em receita separada `check-servidor`.
- AGENTS.md continua a fonte das regras; CLAUDE.md importa `@AGENTS.md`.

## Tried and ruled out
- Adotar o modelo inteiro (código em inglês, squash, docs/adr): descartado pelo usuário; o repo tem 257 commits e 17 decisões no formato próprio.

## Watch out
- O .claude/settings.json do modelo SUBSTITUIU o do projeto e removeu bloqueios (leitura de emissor.db, .cofre, .nfbk, tmp-não-commitar/, servidor/.env, deny de --confirmo-producao) e o hook SessionStart do trail. Mesclar de volta; AGENTS.md proíbe afrouxar.
- Skills/rules/docs do modelo citam docs/adr, architecture.md, [Unreleased], squash e commits em inglês: adaptar todos.
- devbox não existe no Windows desta máquina; adicionar just/gitleaks/pre-commit ao devbox.json só no Linux, do mesmo commit do nixpkgs.
