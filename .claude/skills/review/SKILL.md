---
name: review
description: Revisa o diff da branch atual contra a spec e contra riscos de segurança usando subagentes independentes (spec-reviewer e security-reviewer). Use depois de implementar e antes de abrir PR, ou quando o usuário pedir revisão.
argument-hint: "[NNNN da spec (opcional)]"
---

# /review — revisão independente

1. **Base do diff:** `git merge-base HEAD origin/main` (sem remote: `main`). O escopo é `base...HEAD` + mudanças não commitadas.
2. **Spec:** `$ARGUMENTS`, ou o `NNNN` no nome da branch, ou nenhuma.
3. **Lance em paralelo** (uma única mensagem com as duas chamadas):
   - `spec-reviewer` — passe o caminho da spec (ou "sem spec: revise só correção") e a base;
   - `security-reviewer` — passe a base.
   Os revisores não viram esta conversa; dê a eles só caminhos e base, não sua justificativa do código.
4. **Consolide** em uma lista única ordenada por severidade:
   - **bloqueante** — bug com cenário concreto, requisito não implementado, vulnerabilidade;
   - **importante** — critério de aceite sem teste, risco real;
   - **sugestão** — no máximo 3.
   Cada item: `arquivo:linha` — problema — cenário (entrada → resultado errado). Descarte estilo, preferências e achados que você não consegue confirmar lendo o código.
5. **Não corrija automaticamente.** Pergunte quais corrigir; corrigidos, rode `just check` de novo.
