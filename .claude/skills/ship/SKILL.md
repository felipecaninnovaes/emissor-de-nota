---
name: ship
description: Finaliza a branch atual — verifica, organiza commits Conventional, faz push e abre o PR com gh. Só roda quando o usuário invoca /ship.
argument-hint: "[--draft]"
disable-model-invocation: true
---

# /ship — commit + PR

Opções: $ARGUMENTS

1. **Pré-condições** (pare e relate se alguma falhar):
   - não está em `main`/`master`;
   - `just check` passa;
   - `just scan-secrets` limpo (se o gitleaks estiver disponível);
   - `git status` sem arquivos inesperados — mostre-o ao usuário.
2. **Commits:** o que estiver pendente vira commits Conventional coerentes (título em português, ≤ 72 caracteres), um por etapa, só depois de o usuário conferir. Nunca `--no-verify`; se um hook falhar, corrija a causa. O merge é sem squash: cada commit da branch chega à `main` como está.
3. **CHANGELOG:** garanta a entrada em `## Não publicado` se houver mudança visível para quem usa o app.
4. **Push:** `git push -u origin <branch>` (pedirá confirmação).
5. **PR:** `gh pr create` com
   - título Conventional em português, resumindo a branch;
   - corpo em português: resumo, link da spec, como foi verificado (comandos e resultado), riscos (em especial fiscais: leiaute, numeração, Produção) e o que ficou de fora;
   - `--draft` se o usuário pediu ou se há pendências.
6. Devolva o link do PR. **Não faça merge**: isso é decisão humana.
