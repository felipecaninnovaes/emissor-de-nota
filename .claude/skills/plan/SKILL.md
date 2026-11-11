---
name: plan
description: Transforma uma spec aprovada (ou um pedido pequeno e claro) em plano técnico e lista de tarefas verificáveis em docs/specs/NNNN-slug/. Use antes de implementar qualquer mudança que toque mais de um arquivo ou exija escolher uma abordagem.
argument-hint: "[NNNN da spec | descrição curta]"
---

# /plan — da spec às tarefas

Entrada: $ARGUMENTS

1. **Localize a spec.** Número → `docs/specs/NNNN-*/spec.md`.
   - Spec com `Status: rascunho` → pare e peça aprovação (ou `/spec`).
   - Sem spec e o pedido é pequeno e claro → crie `docs/specs/NNNN-<slug>/` só com `plan.md` e `tasks.md`.
   - Sem spec e o pedido é vago ou muda comportamento público → sugira `/spec`.
2. **Explore.** Leia o código afetado (para varreduras amplas, use o subagente Explore). Anote os padrões a seguir e o código existente que **não** deve ser copiado.
3. **plan.md** a partir de [docs/specs/_template/plan.md](../../../docs/specs/_template/plan.md): abordagem, alternativas descartadas e por quê, módulos afetados, contratos alterados, estratégia de teste ligada a cada requisito, riscos e rollback. Decisão difícil de reverter → marque `→ ADR` e rode `/adr` após a aprovação.
4. **tasks.md** a partir de [docs/specs/_template/tasks.md](../../../docs/specs/_template/tasks.md). Cada tarefa:
   - cabe em um commit e deixa `just check` verde ao terminar;
   - cita os requisitos (R#) que cobre, os arquivos, o teste que a prova e o comando de verificação;
   - está em ordem de dependência (normalmente começa por teste/estrutura).
   Todo R# da spec aparece em pelo menos uma tarefa.
5. **Aprovação.** Apresente abordagem + tarefas em resumo. Ajuste até aprovar.
6. **Próximo passo.** Sugira `/implement NNNN`.
