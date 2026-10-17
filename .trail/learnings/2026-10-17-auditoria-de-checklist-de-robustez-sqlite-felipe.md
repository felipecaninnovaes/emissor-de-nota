---
title: "Auditoria de checklist de robustez (SQLite, backup, TLS, relógio)"
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 2, tokens: 24054 }
touches:
  - docs/escopo.md
---

## Decided
Checklist genérica de NFS-e (RPS/lote) auditada contra o código: WAL, foreign_keys, cofre Argon2id+AES-GCM, alerta de vencimento e estado `enviando` já estão cobertos. Lacunas reais: backup automático + exportação de XMLs (pendente em docs/escopo.md), aviso de desvio de relógio vs Sefin, `min_tls_version(TLS 1.2)` explícito no reqwest (hoje native-tls do sistema).

## Tried and ruled out
Trocar `synchronous` para NORMAL como a checklist sugere: descartado, ADR 0008 mantém FULL. Termos RPS/lote/recibo não se aplicam ao Padrão Nacional (DPS síncrona).

## Watch out
Nenhuma alteração de código foi feita; só auditoria. Não conferi se o alerta de vencimento usa 30 dias nem o duplo clique no botão de emitir.
