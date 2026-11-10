---
title: "E0120 com IM marcador (\"-\") de empresa sem inscrição"
author: Felipe
date: 2026-11-10
branch: corrige-im-placeholder
cost: { minutes: 2, tokens: 13745 }
touches:
  - CHANGELOG.md
  - crates/app/ui/index.html
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/empresas.js
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-db/src/empresas.rs
  - crates/nfse-db/src/importacoes.rs
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/testes.rs
---

## Decided
- `Empresa::im_para_dps` (crates/nfse-db/src/empresas.rs) ignora IM sem caractere alfanumérico diferente de '0' ("-", "0", "000", "."): não vai na DPS. Cobre empresas já gravadas com "-".
- Cadastro da empresa ganhou checkbox `sem_im` ("Sem inscrição municipal"), que desativa e limpa o campo IM e grava a IM como nula. Ao editar, IM marcador abre com a caixa marcada (`semInscricao` em empresas.js espelha a regra do Rust).
- Contorno sem versão nova: Empresas → Mais → Numeração / IM → desmarcar a IM do ambiente.

## Tried and ruled out
- Só tirar o `required` do campo IM: o usuário preferiu uma opção explícita que desativa o campo.

## Watch out
- E0120 = CNC do município emissor sem informações complementares; mesmo uma IM real não pode ir. A flag por ambiente (im_na_dps_producao/homologacao) continua valendo.
- Se mudar a regra de marcador em `im_para_dps`, mude também `semInscricao` em crates/app/ui/js/empresas.js.
