---
title: "Documentação reorganizada: fases 3 e 4 concluídas"
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 9, tokens: 120276 }
touches:
  - AGENTS.md
  - docs/arquitetura.md
  - docs/configuracao.md
  - docs/desenvolvimento.md
  - CLAUDE.md
  - README.md
  - CHANGELOG.md
  - docs/usuario.md
  - docs/deploy.md
  - docs/escopo.md
---

## Decided
- Documentação reorganizada e commitada por etapa (fases 3 e 4): README de apresentação, AGENTS.md (CLAUDE.md só faz `@AGENTS.md`), CHANGELOG a partir das tags, `docs/` com arquitetura, desenvolvimento, configuracao, fiscal, usuario, deploy, escopo e `decisoes/` (12 ADRs). Planos antigos em `docs/_arquivo/`; DIAGNOSTICO-SISTEMA apagado (nada útil que não estivesse no código ou nos learnings).
- Dados do escritório (volumes, município, sistema contábil) não entram na documentação; o histórico do git ainda os tem na versão antiga do Escopo.
- Sem CONTRIBUTING.md (uso interno).

## Tried and ruled out
- Manter `docs-audit.md` e `docs-plan.md` na raiz: eram artefatos do processo e citavam dados do escritório; foram apagados.

## Watch out
- `cargo test --workspace` no Linux: a falha de `identidade_mtls_a_partir_do_pfx` é do OpenSSL 3 com o `.pfx` de teste legado (RC2-40); `openssl pkcs12 ... -legacy` abre. Use `--no-fail-fast` para ver os demais (135 ok, 6 ignorados).
- Script de validação dos links e caminhos citados nos docs foi feito em Python fora do repositório; refazer se a estrutura mudar. `crates/graft/` aparece nos docs mas não existe no checkout (é cache de ferramenta, excluído do workspace).
