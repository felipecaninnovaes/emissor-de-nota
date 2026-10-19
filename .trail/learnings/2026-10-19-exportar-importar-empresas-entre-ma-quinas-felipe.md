---
title: "Exportar/importar empresas entre máquinas (issue #1, ADR 0015)"
author: Felipe
date: 2026-10-19
branch: main
cost: { minutes: 12, tokens: 162088 }
touches:
  - AGENTS.md
  - crates/app/src/comandos/empresas.rs
  - crates/app/src/main.rs
  - crates/app/ui/index.html
  - crates/app/ui/js/componentes.js
  - crates/app/ui/js/empresas.js
  - crates/nfse-db/src/dps.rs
  - crates/nfse-db/src/empresas.rs
  - crates/nfse-db/src/lib.rs
  - crates/nfse-db/src/servicos.rs
---

## Decided
- Só cadastros no `.nfemp` (empresa, serviços ativos, tomadores vinculados, recorrências ativas); sem notas, numeração, certificado ou configurações. Escolha do Felipe entre "transferir", "série por máquina" e "só cadastros".
- Arquivo cifrado com senha própria da exportação via `Cofre::criar` (sal novo, mesmas regras de senha); erros do cofre remapeados porque as mensagens dizem "senha mestra".
- Empresa nova importada: `empresa.conferir_numeracao = 1` (migração 0008); `reservar_dps` recusa até `ajustar_numeracao_pela_sefin` chamar `marcar_numeracao_conferida`. Ajuste manual do número NÃO tira a marca.
- Empresa existente: atualiza dados cadastrais mas mantém ambiente e série (trocar série no destino recomeçaria a numeração e colidiria com a origem).
- Importação tudo ou nada numa transação; chaves de deduplicação: tomador por documento, serviço por nome normalizado, recorrência por (tomador, serviço, descrição).
## Tried and ruled out
- Reusar `salvar_recorrencia`/`importar_servicos` dentro da importação: usam o pool, e o pool tem uma conexão só (trava com a transação aberta). O SQL vai direto na transação; `gravar_tomador`, `gravar_servico`, `SELECT_TOMADOR`, `tomador_de` e `tipo_e_numero` viraram pub(crate).
## Watch out
- `trail check` acusa "cifrar com senha" pelo learning do backup: lá a senha própria foi descartada para o backup automático, não para a exportação.
- Próxima migração passa a ser a 0009.
