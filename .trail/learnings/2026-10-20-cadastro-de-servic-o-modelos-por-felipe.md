---
title: "Cadastro de serviço: modelos por regime e opções avançadas"
author: Felipe
date: 2026-10-20
branch: main
cost: { minutes: 4, tokens: 57698 }
touches:
  - crates/app/ui/estilo.css
  - crates/app/ui/index.html
  - crates/app/ui/js/formularios.js
  - crates/app/ui/js/servicos.js
  - docs/usuario.md
  - crates/app/src/comandos/backup.rs
  - crates/app/src/comandos/cofre.rs
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/main.rs
  - crates/app/ui/js/configuracoes.js
---

## Decided
- `MODELOS` em ui/js/servicos.js preenchem só o que depende do regime: total de tributos, PIS/COFINS próprio (CST 01; Presumido 0,65/3,00, Real 1,65/7,60) e IBS/CBS. Retenções e ISS não mudam (dependem do serviço e do cliente). Só aparecem os modelos do regime da empresa; em serviço novo de ME/EPP ou MEI o único modelo já vem aplicado; fora do Simples o operador escolhe Presumido ou Real.
- Total de tributos, PIS/COFINS próprio e IBS/CBS ficam num `<details id="servico-avancadas">` com resumo no título.
- `TOTAL_POR_REGIME` espelha `conferir_total_tributos` (nfse-core) e barra no campo, ao salvar, a forma que a Sefin rejeitaria (E0710/E0712/E0713).
- A lista de tpRetPisCofins saiu da tela: o tipo vem dos percentuais retidos (mesma tabela de `tipo_retencao_pis_cofins` em nfse-emissor/src/montagem.rs). Sem percentual retido, vale o tipo já guardado no serviço (planilha), como na emissão.

## Tried and ruled out
- Aplicar Presumido sozinho para não optante: errado para Lucro Real (alíquotas bem diferentes); por isso o operador escolhe.

## Watch out
- `validarCampos` ignorava campos sem caixa na tela, o que inclui os de um `<details>` fechado; agora `emUso` os conta e `marcarErro` abre o bloco. Campo dentro de `[hidden]` continua fora.
- No jsdom, `getClientRects()` é sempre vazio: a validação do cliente fica inerte no smoke test; só os `ErroCampo` do salvar aparecem.
