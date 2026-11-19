---
title: "Plano da spec 0003: cancelamento em lote com aviso por e-mail"
author: Felipe
date: 2026-11-19
branch: docs/spec-cancelamento-em-lote
cost: { minutes: 6, tokens: 121634 }
touches:
  - docs/specs/0003-cancelamento-em-lote/plan.md
  - docs/specs/0003-cancelamento-em-lote/tasks.md
---

## Decided
O lote roda na tela, com uma chamada do comando `cancelar` por nota. O comando ganha `avisar` e passa a devolver um resultado classificado (cancelada, recusada ou sem_confirmacao). O "sem confirmação" vira a variante `ErroEmissao::CancelamentoIncerto`, com o mesmo texto do erro. `{motivo}` e `{justificativa}` vêm do evento gravado: o nfse-db devolve `xml_cancelamento` e o nfse-xml o interpreta (`motivo_do_cancelamento`). `ConfigSmtp` ganha assunto e corpo do cancelamento com `serde(default)`, sem migração. Sem limite de notas por lote além do teto de 500 da lista; aviso acima de 20.

## Tried and ruled out
Comando único `cancelar_lote` no backend: não daria para mostrar o andamento. Classificar o erro pelo texto no JS: frágil. Colunas novas para o motivo: exigiriam migração e não cobririam a nota cancelada no portal.

## Watch out
`selecaoEmTabela` (tabela.js) já pula as caixas desabilitadas: desabilite as das notas que não estão autorizadas. `obter_email` já devolve `configurado`. Com três notas no mesmo teste, a Sefin simulada precisa de chaves distintas. A spec só muda na T7, com registro no histórico.
