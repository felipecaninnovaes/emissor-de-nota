---
title: "Reter abaixo de R$ 10,00 por escolha na revisão"
author: Felipe
date: 2026-10-29
branch: main
cost: { minutes: 5, tokens: 126380 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/emissao.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/emitir.js
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/montagem.rs
  - crates/nfse-emissor/src/testes.rs
  - docs/fiscal.md
  - docs/usuario.md
  - Cargo.lock
---

## Decided
- A dispensa de retenção de até R$ 10,00 pode ser desligada por nota: `ReterNota::irrf_ate_o_limite` e `contribuicoes_ate_o_limite` (separados), usados por `federais_com_escolha` em `montar_dps`. `federais_da_nota` continua sendo a versão sem escolha (dispensa sempre).
- O limite vale para o total pago no período (várias notas do mesmo tomador), que o emissor não soma: quem decide é o operador, na revisão (fieldset `#revisao-limite`, só aparece quando há retenção dispensável).
- `retencoes_dispensaveis` devolve os valores (struct `RetencoesDispensaveis`); o texto do aviso fica em `previa_dps` (app), que não avisa o que o operador mandou reter.
- O PedidoDto recebe `reter_irrf_ate_o_limite` e `reter_contribuicoes_ate_o_limite` (serde default false); "Revisar" zera `estado.reter`, e com ele a escolha.

## Tried and ruled out
- Somar automaticamente as notas do período por tomador: não pedido; o pagamento pode não coincidir com a emissão.

## Watch out
- O `trail check` acusa o teste `retencao_de_ate_dez_reais_e_dispensada` por valor retido de 10,00: é proposital (testa a dispensa).
