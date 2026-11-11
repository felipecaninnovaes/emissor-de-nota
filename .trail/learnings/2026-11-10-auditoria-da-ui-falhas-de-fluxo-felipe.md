---
title: "Auditoria da UI: falhas de fluxo e produtividade (nov/2026)"
author: Felipe
date: 2026-11-10
branch: main
cost: { minutes: 5, tokens: 234019 }
touches:
  - crates/app/ui/index.html
  - crates/app/ui/js/emitir.js
  - crates/nfse-core/src/modelo.rs
  - crates/nfse-db/src/importacoes.rs
  - crates/nfse-db/src/tomadores.rs
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/testes.rs
  - crates/nfse-pdf/src/dados.rs
  - crates/nfse-pdf/src/lib.rs
  - crates/nfse-pdf/src/testes.rs
---

## Decided
Auditoria só de leitura da UI (crates/app/ui/index.html e os 18 módulos de ui/js), com conferência pontual no backend. Nada foi alterado nem testado no app. Achados por prioridade, ainda sem correção:
1. Crítico: durante o envio, "Revisar" continua ativo e o Ctrl+Enter (preferencias.js) clica nele quando "Emitir nota" está desabilitado; a revisão nova reabilita o botão (emitir.js, fim de `revisar`) e o Ctrl+Enter seguinte emite de novo. O comando `emitir` não tem trava contra emissão simultânea.
2. "Corrigir e emitir" de nota rejeitada vinda de recorrência sai com `Origem::Manual` (comandos/emissao.rs); o lote tende a manter a recorrência pendente no mês.
3. Planilha lida para uma empresa não é relida ao trocar a empresa no topo (irPara("planilha") só redesenha).
4. Campo de valor: "1.500" sem vírgula vira 1,50 (valorDigitado/numeroDigitado).
5. Texto na busca de tomador sem escolher na lista vira nota sem tomador; no diálogo de recorrência, Enter antes da lista chegar salva sem tomador.
6. Enter duplo em diálogo: `requestSubmit` com o botão desabilitado ainda submete (Chromium), repetindo o salvar.
7. "Bloquear" e fechar a janela não consultam `envioEmCurso()` (só a atualização consulta); main.rs não trata CloseRequested.
Outros: lote sem "parar"; motivos das falhas somem ao recarregar o lote; "Ler de novo" relê bytes da memória; Configurações recarrega por cima do não salvo; tomador sem excluir e empresa arquivada sem reativar; `alert()` em servicos.js.

## Tried and ruled out
Não são problemas (já tratados, não refazer): estados vazios com botão em todas as listas; validação junto do campo; revisão descartada quando o formulário muda; respostas antigas de `previa_dps` e do combobox ignoradas por sequência; guarda de atualização durante envio; confirmação antes de excluir, restaurar e passar a Produção.

## Watch out
A correção do item 1 precisa de guarda nos dois lados: `revisar()` e o handler de confirmação saindo cedo com `emissaoEmCurso`, e ignorar `ev.repeat` no atalho. Os itens 1 e 6 são dedução do código, não reprodução no app. O item 2 não teve rastreado como o lote calcula a situação do mês (`no_mes`). `bloquear_cofre` espera a operação em curso (trava de escrita) antes de fechar, então a nota em envio termina, mas a fila do lote morre com o reload.
