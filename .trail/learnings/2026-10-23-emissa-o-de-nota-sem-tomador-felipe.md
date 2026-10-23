---
title: "Emissão de nota sem tomador: onde está e lacunas"
author: Felipe
date: 2026-10-23
branch: main
cost: { minutes: 1, tokens: 2170 }
---

## Decided
- Nota sem tomador = `Dps.tomador: None`. O XML omite `<toma>` (nfse-xml/src/dps.rs, `if let Some(t) = &dps.tomador`); `Dps::validar` só confere o tomador se ele existe; `ReterNota::efetiva` (nfse-emissor/src/lib.rs) não retém nada sem tomador; o PDF imprime "TOMADOR/ADQUIRENTE DA OPERAÇÃO NÃO IDENTIFICADO" (nfse-pdf/src/dados.rs + lib.rs).
- Tela de emissão, recorrências, lote, planilha, cópia e transferência já aceitam tomador ausente.

## Tried and ruled out
- Procurar "sem_tomador" no código: não existe esse símbolo; o caso é sempre `tomador: None` / `?? "sem tomador"` no JS.

## Watch out
- `valida_no_xsd_oficial` (nfse-xml/src/dps.rs) não tem caso sem `<toma>`: falta cobertura XSD NT 004/NT 009.
- As caixas "reter federais/ISS" da revisão (emitir.js, `previa.retencao_condicional`) não olham se há tomador, e `Dps::validar` não recusa retenção sem tomador: a rejeição viria só da Sefin, depois de reservar número. Não verificado no app.
