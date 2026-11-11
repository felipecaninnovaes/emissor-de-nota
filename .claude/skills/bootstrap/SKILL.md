---
name: bootstrap
description: Inicializa um projeto novo criado a partir deste modelo — entrevista sobre stack e identidade, preenche todos os TODO(bootstrap), configura justfile, devbox, CI, Dependabot e licença, e deixa `just check` verde.
disable-model-invocation: true
---

# /bootstrap — do modelo ao projeto

Objetivo: ao final, `grep -rn "TODO(bootstrap)" . --exclude-dir=.git --exclude-dir=bootstrap --exclude=session-start.sh` não retorna nada e `just check` passa com verificações **reais**.

## 1. Diagnóstico (sem perguntar)
- Liste os `TODO(bootstrap)` restantes.
- Detecte código existente (manifestos como `package.json`, `pyproject.toml`, `go.mod`, `Cargo.toml`, `*.csproj`, `build.gradle*`) e infira stack e comandos. O que for inferido vira opção recomendada na entrevista, não pergunta aberta.
- Verifique ferramentas: `devbox`, `just`, `gh`, `git`.

## 2. Entrevista
Siga [questionnaire.md](questionnaire.md): rodadas de até 4 perguntas com AskUserQuestion, opção recomendada primeiro. Pule perguntas que o diagnóstico já respondeu. Use [presets.md](presets.md) para sugerir comandos por stack.

## 3. Aplicar (mostre o plano de mudanças antes e aguarde ok)
| Arquivo | O que fazer |
|---|---|
| `CLAUDE.md` | Nome, descrição, remover aviso de bootstrap. Adicionar só gotchas reais da stack (ex.: "rode X antes de Y"). Manter < 100 linhas. |
| `justfile` | Implementar `setup fmt fmt-check fmt-file lint test build` com os comandos da stack. `test *args` deve repassar `{{args}}` para rodar um único teste. Manter os nomes das receitas. |
| `devbox.json` | `devbox add` do runtime/ferramentas com versão fixada (ex.: `nodejs@22`, `python@3.13`, `go@1.24`). |
| `.gitignore`, `.editorconfig` | Entradas e indentação da stack. |
| `.github/dependabot.yml` | Descomentar/adicionar o ecossistema da stack. |
| `.github/workflows/ci.yml` | Cache da stack se útil; serviços (DB) se os testes exigirem. |
| `.pre-commit-config.yaml` | Opcional: hook `repo: local` chamando `just fmt-check`. |
| `.env.example` | Variáveis de ambiente necessárias. |
| `LICENSE`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.github/CODEOWNERS`, `.github/ISSUE_TEMPLATE/config.yml` | Titular, ano, contato, owner/repo do GitHub. |
| `README.md` | Reescrever para o projeto (o que é, como rodar, como contribuir). Mover o conteúdo sobre o modelo para uma seção curta "Desenvolvimento com IA" com link para `docs/workflow.md`. |
| `docs/architecture.md` | Primeira versão: contexto, componentes previstos, integrações externas. |
| `docs/adr/` | ADR `0003` com a escolha de stack (via fluxo do `/adr`, já `aceito`). |
| `.claude/rules/` | Regras por caminho só para convenções da stack que o linter não cobre. |
| `CHANGELOG.md` | Manter `[Unreleased]`. |

Se o usuário pediu esqueleto mínimo: crie o menor código possível com **um teste real** para provar que `just test` executa algo.

## 4. Verificar
1. Sem repo git: `git init -b main`.
2. `devbox install` (se houver devbox) e `just setup`.
3. `just check` deve passar — mostre a saída.
4. Teste o contrato: `just test <um arquivo de teste>` roda só aquele teste.
5. Grep de `TODO(bootstrap)` vazio (o comando do objetivo, no topo).

## 5. Primeiro commit
Na `main` vazia é permitido (o hook só bloqueia quando já há commits). O pre-commit bloqueia commits na main, então só este usa `SKIP=no-commit-to-branch`:
`SKIP=no-commit-to-branch git commit -m "chore: bootstrap project from template"`

## 6. GitHub (outward-facing — pergunte antes de cada passo)
Ofereça executar com `gh`, mostrando o comando:
- criar o repositório e fazer o push inicial da `main`;
- ruleset na `main`: PR obrigatório, status checks `check` e `pr-title`, só squash merge, bloquear force push;
- habilitar Dependabot alerts/updates, secret scanning + push protection, private vulnerability reporting;
- `delete_branch_on_merge`.

Encerre com: o que foi configurado, o que ficou pendente e o próximo passo sugerido (`/spec` da primeira feature).
