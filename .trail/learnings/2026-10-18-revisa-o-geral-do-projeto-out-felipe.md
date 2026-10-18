---
title: "Revisão geral do projeto (out/2026): achados confirmados"
author: Felipe
date: 2026-10-18
branch: main
cost: { minutes: 1, tokens: 18581 }
---

## Decided
- Revisão só leitura em 5 frentes (emissão/Sefin, cripto, banco/comandos, modelo/XML/PDF, UI). Nada alterado ainda.
- Confirmados no código: (1) emitir(): se gravar() falha após 200 da Sefin, DPS fica `enviando` e o operador vê erro (nfse-emissor/src/lib.rs ~652); (2) 404 em consultar_dps logo após timeout libera a DPS para reenvio na hora (reconciliar_com → liberar_para_reenvio); (3) consultar_dps trata 2xx sem chaveAcesso como "não existe" (nfse-nacional/src/lib.rs ~196); (4) falha em gerar_dps/assinar após reservar deixa DPS `reservada` órfã que bloqueia recorrência/linha de planilha; (5) Validador::texto não aplica o padrão TSString do XSD (Latin-1, sem espaço nas pontas) → rejeição de schema depois de reservar número; (6) restaurar_backup não checa colisão do `.cofre` do destino; (7) reqwest sem redirect Policy::none nem min TLS 1.2; (8) planilha XLSX: célula % vira fração e `{:.2}` trunca (planilha.rs ~196); CNPJ testado antes de CPF em documento_da_planilha; (9) UI: selects de mês ativos durante lote + sem try/finally em emitirEmSequencia (app.js ~1866).
## Tried and ruled out
- Sem XSS (esc() consistente), sem SQL injection, todos os comandos registrados e com exigir_cofre, sem f64 em dinheiro, sem log de segredos, CNPJ alfanumérico correto.
## Watch out
- Ao corrigir (2) e o "Verificar" em DPS `enviando` recente: exigir idade mínima (>90 s timeout + margem) antes de liberar para reenvio.
