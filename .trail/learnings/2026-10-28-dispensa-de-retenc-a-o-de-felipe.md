---
title: "Dispensa de retenção de até R$ 10,00 (IRRF e PIS/COFINS/CSLL)"
author: Felipe
date: 2026-10-28
branch: main
cost: { minutes: 5, tokens: 94947 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/emissao.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/montagem.rs
  - crates/nfse-emissor/src/testes.rs
  - crates/nfse-xml/src/dps.rs
  - docs/fiscal.md
  - docs/usuario.md
  - Cargo.lock
  - crates/app/Cargo.toml
---

## Decided
- `federais_da_nota` (nfse-emissor/src/montagem.rs) calcula como antes (`calcular_federais`) e, se o IRRF ou a soma PIS+COFINS+CSLL (vRetCSLL) ficar entre 0,01 e 10,00, refaz o cálculo com aquele grupo tirado da tributação, como `tributacao_da_nota` faz para ente público (`tipo_retencao` vira 0; PIS/COFINS próprios continuam).
- Limite "igual ou inferior" a R$ 10,00 (Lei 9.430 art. 67; Lei 10.833 art. 31 § 3º): com 4,65%, 215,00 dá 10,00 e é dispensado; com 1,5%, 666,00 dá 9,99. O usuário pediu "menor que 10" mas citou 215 como teto, o que só fecha com ≤.
- Vale também para o vRetCSLL vindo do tipo do cadastro (sem percentual) e para valores fixos de IRRF/CSLL.
- A revisão (`previa_dps`) avisa com `retencoes_dispensadas`; o aviso antigo "até R$ 215,05" saiu (não disparava mais).

## Tried and ruled out
- Testar pela base (215 / 666): depende das alíquotas do serviço; o critério legal é o valor retido.

## Watch out
- Testes antigos com retenção pequena (PIS 0,65% sobre 650; 1.000 com só PIS) passaram a ser dispensados; subi os valores. Teste novo de retenção precisa de valor retido acima de 10,00.
- INSS (CP) não entra na dispensa.
