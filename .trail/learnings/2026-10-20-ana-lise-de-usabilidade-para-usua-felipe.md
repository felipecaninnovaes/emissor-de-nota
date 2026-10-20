---
title: Análise de usabilidade para usuário leigo (out/2026)
author: Felipe
date: 2026-10-20
branch: main
cost: { minutes: 1, tokens: 63721 }
touches:
  - docs/usuario.md
---

## Decided
Análise só leitura da UI (index.html, ui/js, docs/usuario.md, nfse-nacional/src/mensagens.rs). Prioridades sugeridas ao usuário, nada implementado ainda:
1. Checklist de primeiros passos em Emitir (empresa → certificado → numeração → serviço → tomador → pasta/backup).
2. Faixa forte de "modo teste" em Homologação; aviso persistente sem pasta de backup; lembrete de anotar a senha mestra.
3. Botões de correção junto das orientações de erro da Sefin (ex.: E0014 → ajustar numeração e reemitir) e "Verificar agora" no resultado sem resposta definitiva.
4. Maior esforço: diálogo de serviço com modelos por regime e blocos PIS/COFINS próprio, IBS/CBS e total de tributos em "Opções avançadas"; tp_ret deduzido dos percentuais.
Outros: município pelo nome/CEP em vez de IBGE, resumo "cliente desconta / você recebe" na revisão, busca e filtro em Notas emitidas, presets SMTP, agrupar Lote/Planilha/Recorrentes.

## Tried and ruled out
Nada descartado; a UI já tem estados vazios em notas.js, erros junto do campo e orientação por código em mensagens.rs (não refazer).

## Watch out
"Verificar agora" precisa respeitar a idade mínima da DPS `enviando` (ver learning da revisão geral). O empresa-municipio vem com 3511102 preenchido no index.html. A restauração de backup hoje é troca manual de emissor.db + emissor.cofre.
