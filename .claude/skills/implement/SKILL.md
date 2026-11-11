---
name: implement
description: Executa as tarefas de docs/specs/NNNN-slug/tasks.md uma a uma, com teste antes, verificação e commit por tarefa, numa branch de feature.
argument-hint: "[NNNN]"
disable-model-invocation: true
---

# /implement — execução das tarefas

Spec: $ARGUMENTS

1. **Carregue** `docs/specs/NNNN-*/` (`spec.md` se existir, `plan.md`, `tasks.md`). Sem `tasks.md` → pare e sugira `/plan`.
2. **Branch.** Se estiver em `main`: `git switch -c feat/NNNN-<slug>` (`fix/` para correção). Se já estiver numa branch da spec, continue nela.
3. **Para cada tarefa não marcada, em ordem:**
   1. Escreva o teste que prova a tarefa e rode-o: ele deve **falhar pelo motivo certo**. Para muitos critérios de aceite de uma vez, delegue ao subagente `test-writer`.
   2. Implemente o mínimo para passar, seguindo os padrões existentes no código.
   3. `just test <alvo>`; depois `just check`.
   4. Marque `[x]` em `tasks.md`; se desviou do plano, anote em uma linha.
   5. Mostre o resultado e, **depois que o usuário conferir**, commit Conventional com título em português, ex.: `feat(emissor): recusa nota com competência futura`, com `Refs: docs/specs/NNNN-<slug>` no corpo. Só os arquivos alterados, pelo nome.
4. **Desvio relevante** (requisito inviável, mudança de contrato, dependência nova, invariante em risco): pare, explique e pergunte. Atualize `plan.md` antes de seguir.
5. **Fechamento:** `just check` completo, atualize `CHANGELOG.md` `## Não publicado` se a mudança for visível para quem usa o app, rode `/review`.
6. **Relate:** tarefas concluídas, evidência (comando + final da saída), achados da revisão e pendências. Sugira `/ship`.
