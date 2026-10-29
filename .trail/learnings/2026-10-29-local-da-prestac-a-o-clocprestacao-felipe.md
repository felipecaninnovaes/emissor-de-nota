---
title: Local da prestação (cLocPrestacao) na emissão
author: Felipe
date: 2026-10-29
branch: main
cost: { minutes: 7, tokens: 49624 }
touches:
  - CHANGELOG.md
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/comandos/notas.rs
  - crates/app/src/comandos/recorrencias.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/consultas.js
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/notas.js
  - crates/nfse-emissor/src/copia.rs
  - crates/nfse-emissor/src/importacao.rs
---

## Decided
- `PedidoEmissao::local_prestacao: Option<String>` (IBGE); `montar_dps` usa-o em `servico.municipio_prestacao`, senão o município da empresa. `Dps::validar` já confere 7 dígitos, antes de reservar número.
- `Refazer::local_prestacao` só vem quando a DPS antiga tinha município de prestação diferente do `cLocEmi`; refazer, copiar e substituir o trazem de volta.
- Tela: campos `#local-cep` (consulta de CEP de `consultas.js`, com "Não sei o CEP") e `#local-prestacao` (IBGE); o JS manda `null` se vazio ou igual ao município da empresa. O campo é limpo depois de cada nota autorizada, como a obra.
- O emissor não mexe no ISS pelo local: a Sefin apura o município de incidência (exceções do art. 3º da LC 116).

## Tried and ruled out
- Local padrão no cadastro do serviço e na recorrência: precisaria de coluna nova (recorrência) e não foi pedido; recorrência, lote e planilha usam o município da empresa.
- `cPaisPrestacao` (exterior): não tratado.

## Watch out
- Ainda não testado na Sefin com município diferente: regras de ISS do outro município (alíquota, retenção) podem gerar rejeição nova.
