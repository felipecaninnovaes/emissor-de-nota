---
title: "cClassTrib embutido, par CST validado e fonte das tabelas do IBS/CBS"
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 30, tokens: 126672 }
touches:
  - Cargo.lock
  - crates/app/src/comandos/servicos.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/formularios.js
  - crates/app/ui/js/servicos.js
  - crates/nfse-core/src/lib.rs
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-core/src/tabela_codigos.rs
  - crates/nfse-db/src/servicos.rs
---

## Decided
- A tabela do cClassTrib vem da página Classificação Tributária do Portal DFe (dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria): não há planilha, mas o HTML traz o JSON em `var dadosOriginais = [...]` (curl funciona, 173 códigos em 19/10/2026). Embutida em nfse-core/dados/class-trib.tsv (módulo class_trib), gerada por `examples/gerar_class_trib`.
- Cada cClassTrib tem um só CST: o par é validado em Dps::validar e Servico::validar, e escolher o código na tela preenche o CST. Código fora da tabela não bloqueia (mesma regra da NBS); só o par errado bloqueia.
- Planilha de serviços: CST em branco com cClassTrib conhecido assume o CST da tabela (antes virava 000).
- A tabela de cIndOp (39 códigos) está em dfe-portal.svrs.rs.gov.br/CFF/TabelaIndicadoresDosLocaisDeOperacao, HTML estático.

## Tried and ruled out
- Procurar um cálculo do cIndOp/CST/cClassTrib no emissor: não existe; são códigos informados no cadastro. Quem calcula alíquota, redução e vIBS/vCBS é a Sefin, a partir do cClassTrib (200029 = 60% de redução no IBS e na CBS).

## Watch out
- O que o emissor precisa calcular é o PIS/COFINS da DPS: o Emissor Web manda vBCPisCofins, pAliqPis, pAliqCofins, vPis e vCofins e a Sefin desconta vPis e vCofins da base do IBS/CBS. Sem eles a base sai só menos o ISS (comparado com XML real em 19/10/2026).
- O cIndOp padrão do select (100301) não é o de serviço de saúde presencial (030101).
- Em Windows com autocrlf, arquivos .rs e .js do repositório estão em CRLF no disco; edições por script devem preservar o fim de linha.
