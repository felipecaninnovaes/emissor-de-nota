---
title: "Reorganização da documentação: auditoria e plano (fases 1 e 2)"
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 6, tokens: 110762 }
touches:
  - docs-audit.md
  - docs-plan.md
  - docs/DIAGNOSTICO-SISTEMA.md
---

## Decided
- Reorganização da documentação em 4 fases (inventário, plano, execução, validação). Fases 1 e 2 prontas: `docs-audit.md` e `docs-plan.md` na raiz. A Fase 3 só começa após o "aprovado" do usuário; nada foi movido nem apagado ainda.
- Estrutura proposta: README curto, CHANGELOG, `docs/` (arquitetura, desenvolvimento, configuracao, fiscal, usuario, deploy, escopo, decisoes/ com 12 ADRs, _arquivo/). Sem `api.md` (não há API própria).
- Documentos históricos (ESTADO, PLANO-*, ROTEIRO-*, DIAGNOSTICO-SISTEMA) vão para `docs/_arquivo/`, não são deletados.

## Tried and ruled out
- Tratar `docs/DIAGNOSTICO-SISTEMA.md` como documentação viva: está desatualizado (comandos.rs monolítico, versão 0.2.1) e vários itens foram corrigidos ou descartados por decisão.

## Watch out
- Divergências doc x código achadas: Escopo diz auto-update "pendente" e "feito"; CNPJ hoje é BrasilAPI (docs citam publica.cnpj.ws); variáveis de descrição e migração 0006 (tomador que retém) não documentadas; ESTADO.md aponta para comandos.rs que virou `comandos/`.
- O arquivo do Escopo tem travessão e espaços no nome; `git ls-files | grep .md` não o lista por causa do escape. Use `git -C` e glob entre aspas; no zsh, `--include=*.rs` sem aspas falha.
- `cargo test --workspace` não foi rodado; um learning anterior diz que falha no Linux em `identidade_mtls_a_partir_do_pfx`.
