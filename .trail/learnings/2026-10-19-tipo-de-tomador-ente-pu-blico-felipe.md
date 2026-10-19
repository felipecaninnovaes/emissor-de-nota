---
title: "Tipo de tomador (ente público), tpEnteGov/tpOper e retenção por esfera (issue #2, etapa 1)"
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 109, tokens: 297799 }
touches:
  - CHANGELOG.md
  - Cargo.lock
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/comandos/tomadores.rs
  - crates/app/ui/estilo.css
  - crates/app/ui/index.html
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/servicos.js
  - crates/app/ui/js/tomadores.js
  - crates/nfse-core/src/modelo.rs
---

## Decided
- O vínculo empresa × tomador (`empresa_tomador.tipo`, migração 0009) guarda o tipo: privado, federal (União), estadual, distrito_federal ou municipal. Só privado e União retêm PIS/COFINS/CSLL; estado, DF e município retêm IRRF/INSS (IN RFB 1.234/2012; sem convênio, art. 33 da Lei 10.833/2003). `tributacao_da_nota` descarta CSLL/PIS/COFINS nesses casos, mesmo com serviço "em toda nota".
- O tipo vira `tpEnteGov` (1 a 4) e `tpOper` (sempre 1, fornecimento com pagamento posterior) em `IBSCBS`, só para tomador público; `indFinal` agora vai sempre (0). Conferido contra a nota do Portal Nacional e contra `TSRTCTpOper`/`TSRTCTpEnteGov` do XSD em `xsd/`; ordem: indFinal, cIndOp, tpOper, gRefNFSe, tpEnteGov, indDest. Os dois campos são opcionais no XSD.
- A revisão da nota mostra retenções, PIS/COFINS próprios, base estimada do IBS/CBS (sem ISS quando a alíquota não está no serviço) e o XML da DPS sem assinatura.

## Tried and ruled out
- Um tipo único "estado ou município": não dá para preencher `tpEnteGov`, que distingue Estado, DF e Município. O valor gravado `estadual_municipal` (versão de teste) é lido como município.
- Editar o comentário da migração 0009 depois de aplicada: muda o checksum do sqlx; foi desfeito.
- Campo do formulário do tomador chamado `tipo`: colide com o seletor CNPJ/CPF (`name="tipo"`); o novo se chama `tipo_tomador`.

## Watch out
- PIS/COFINS "próprios" e "retidos" são campos diferentes no serviço; com os dois preenchidos vale só o retido. A confusão entre eles deu a primeira DPS sem `vPis`/`vCofins`.
- Scripts que editam arquivos aqui precisam de `PYTHONUTF8=1` (o stdin do Python no Windows é cp1252) e de preservar CRLF/LF do arquivo.
