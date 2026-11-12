---
title: "Chave PIX da empresa e variável {pix}"
author: Felipe
date: 2026-11-12
branch: feat/chave-pix-da-empresa
cost: { minutes: 116, tokens: 1491567 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/empresas.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/empresas.js
  - crates/app/ui/js/util.js
  - crates/nfse-core/src/lib.rs
  - crates/nfse-core/src/pix.rs
  - crates/nfse-db/migrations/0010_empresa_chave_pix.sql
  - crates/nfse-db/src/empresas.rs
  - crates/nfse-db/src/transferencia.rs
---

## Decided
- `nfse_core::ChavePix` (crates/nfse-core/src/pix.rs) valida e normaliza os 5 formatos do DICT: CPF/CNPJ sem pontuação (CNPJ alfanumérico aceito), e-mail em minúsculas (até 77), telefone `+55DDNÚMERO`, chave aleatória (UUID) em minúsculas.
- Telefone só com `+` na frente: 11 dígitos sem `+` caem no CPF, e a mensagem de erro lembra do +55. Evita confundir telefone com CPF.
- Coluna `empresa.chave_pix` pela migração 0010 (tabela `empresa` não é recriada por ajustes.rs; a regra de ajustes vale só para `dps`).
- `{pix}` e as demais variáveis valem na descrição e nas informações complementares (`montar_dps`); sem chave cadastrada a variável sobra e `validar` barra com o motivo, uma vez só para os dois campos.
- `.nfemp`: `chave_pix` com `#[serde(default)]`; na importação de empresa existente, `COALESCE(?, chave_pix)` para arquivo antigo não apagar a chave.
- Erro da chave volta para junto do campo: a tela captura mensagens que começam com "chave PIX" e vira `ErroCampo("chave_pix")`.

## Tried and ruled out
- Compartilhar `CARGO_TARGET_DIR` entre o checkout principal e uma worktree com outra versão de um crate: os doctests (`nfse-db`) falharam com import não resolvido. Na worktree, use a `target` própria (a primeira compilação leva ~20 min no Windows por causa do OpenSSL e SQLCipher).

## Watch out
- Rodar o app com uma migração nova aplica-a no banco real em %APPDATA%; versões sem ela recusam abrir o banco (sqlx: migração aplicada que falta). Faça backup de emissor.db e emissor.cofre antes de testar uma branch com migração.
- Texto entre chaves `{algo}` nas informações complementares agora é barrado como variável desconhecida.
