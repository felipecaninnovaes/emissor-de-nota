---
title: Correções de segurança do DIAGNOSTICO-SISTEMA (fase 1)
author: Felipe
date: 2026-10-17
branch: main
cost: { minutes: 7, tokens: 20946 }
touches:
  - crates/app/src/comandos.rs
  - crates/nfse-db/src/lib.rs
  - docs/DIAGNOSTICO-SISTEMA.md
---

## Decided
- Comandos IPC de dados em crates/app/src/comandos.rs chamam `exigir_cofre(&estado)` (cofre aberto) antes de tocar no banco.
- `abrir_arquivo` recebe `estado`, canonicaliza o caminho, exige que fique dentro de `pasta_saida` e só aceita pdf/xml/csv (pastas liberadas).
- Argon2 em `criar_cofre`/`abrir_cofre` (nfse-db/src/lib.rs) roda em `tokio::task::spawn_blocking`.

## Tried and ruled out
- Item 1.3 do diagnóstico (feature `zeroize` no rsa 0.9): desnecessário, a chave já é zerada ao descartar.
- Item 1.6 (lock preso em rede): o RwLock do tokio permite leitores concorrentes; só bloqueia escritores (definir_pasta, abrir_cofre). Trocar por Arc<Emissor> exigiria refatorar `definir_pasta` (usa as_mut); adiado.

## Watch out
- docs/DIAGNOSTICO-SISTEMA.md tem afirmações não verificadas; confira no código antes de agir.
- O botão "abrir pasta" da UI passa o valor digitado em #pasta-saida; se não estiver salvo e fora da pasta configurada, agora dá erro.
- Pendentes: 1.4 (validar cadeia da assinatura da Sefin; testar em homologação), spawn_blocking de RSA/PDF, itens da Fase 2.
