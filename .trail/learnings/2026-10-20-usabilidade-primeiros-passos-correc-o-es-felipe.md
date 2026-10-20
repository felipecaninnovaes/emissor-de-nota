---
title: "Usabilidade: primeiros passos, correções de rejeição, modo teste e restauração"
author: Felipe
date: 2026-10-20
branch: main
cost: { minutes: 12, tokens: 191539 }
touches:
  - crates/app/src/comandos/backup.rs
  - crates/app/src/comandos/cofre.rs
  - crates/app/src/comandos/emissao.rs
  - crates/app/src/main.rs
  - crates/app/ui/estilo.css
  - crates/app/ui/index.html
  - crates/app/ui/js/configuracoes.js
  - crates/app/ui/js/emitir.js
  - crates/app/ui/js/empresas.js
  - crates/app/ui/js/navegacao.js
---

## Decided
- Primeiros passos em Emitir (`PASSOS` em ui/js/emitir.js): empresa → certificado → numeração conferida → serviço; atualiza em `renderizarServicosEmitir`. Sem empresa, o app abre em Emitir (não mais em Empresas).
- Botões de correção por código de rejeição em `CORRECOES` (emitir.js), espelhando `nfse-nacional/src/mensagens.rs`. E0014 chama `ajustar_numeracao` e deixa a revisão aberta para reenviar.
- `ResultadoDto` ganhou `dps_id` (incerta) para o "Verificar agora" (comando `reconciliar`, que já respeita a janela de minutos).
- `trocarEmpresa` só limpa o formulário quando a empresa muda (`empresaDaEmissao`); a mesma recarregada mantém o digitado e descarta a revisão. `renderizarServicosEmitir` mantém o serviço escolhido; a descrição só acompanha o cadastro se não foi editada.
- Restauração de verdade: `restaurar_e_reiniciar` decifra para `restauracao-pendente.db` na pasta de dados e reinicia; `aplicar_restauracao_pendente` (no setup do main.rs, antes de abrir o banco) troca os arquivos com rollback; `restaurado.marca` faz `exigir_conferencia_de_numeracao` em todas as empresas ao abrir o cofre.

## Tried and ruled out
- Trocar os arquivos do banco com o app aberto: no Windows o arquivo aberto não pode ser renomeado; por isso a troca é na abertura seguinte.
- Reenviar automaticamente depois do E0014: ficou em dois cliques (ajustar, depois Emitir nota) para a emissão continuar explícita.

## Watch out
- Ciclos de import entre emitir.js, empresas.js, servicos.js e navegacao.js: código de topo de módulo não pode usar const/let de outro módulo do ciclo (TDZ). As funções novas só rodam depois de abrir o cofre.
- Smoke test da UI sem abrir o app: jsdom no scratchpad, carregando index.html + app.js com `window.__TAURI__.core.invoke` falso. Não sobrescrever o setTimeout global com o do jsdom (recursão).
- Mover o `emissor.db` sem o `-wal`/`-shm` corrompe ou mistura bancos; a troca leva os três e o `.cofre`.
