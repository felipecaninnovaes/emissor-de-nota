# 0008 — SQLite com `synchronous = FULL`

Estado: aceita (17/10/2026).

**Contexto.** O modo WAL aceitaria `NORMAL`, mais rápido, com risco de perder as últimas transações em queda de energia.

**Decisão.** Manter `SqliteSynchronous::Full` (`crates/nfse-db/src/lib.rs`).

**Por quê.** Durabilidade da numeração e do estado `enviando`: perder isso pode repetir número ou esconder uma DPS enviada.
