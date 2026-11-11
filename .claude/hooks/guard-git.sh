#!/usr/bin/env bash
# PreToolUse (Bash): protege a main e o histórico.
# - bloqueia commit direto em main/master (exceto o primeiro commit do repositório)
# - bloqueia push para main/master e force push
# - bloqueia --no-verify (pular hooks)
# Confirmações (git push em geral, reset --hard, rm -rf...) ficam nas regras "ask" do settings.json.
source "$(dirname "$0")/_lib.sh"
read_input

cmd="$(json_get .tool_input.command)"
printf '%s' "$cmd" | grep -Eq '(^|[^[:alnum:]_-])git([[:space:]]|$)' || exit 0

cwd="$(json_get .cwd)"
[ -z "$cwd" ] && cwd="$(project_dir)"

if printf '%s' "$cmd" | grep -Eq -- '--no-verify'; then
  block "Bloqueado: não pule os git hooks (--no-verify). Corrija o que o hook apontou."
fi

branch="$(git -C "$cwd" symbolic-ref --short -q HEAD 2>/dev/null)"
has_commits=1
git -C "$cwd" rev-parse -q --verify HEAD >/dev/null 2>&1 || has_commits=0
# "git" seguido de opções globais (-C dir, -c k=v, --no-pager...) antes do subcomando
GIT='git([[:space:]]+(-[Cc][[:space:]]+[^[:space:]]+|-[^[:space:]]+))*[[:space:]]+'
creates_branch=0
printf '%s' "$cmd" | grep -Eq 'git[[:space:]]+(checkout[[:space:]]+-[bB]|switch[[:space:]]+-[cC])' && creates_branch=1

# commit direto na main
if printf '%s' "$cmd" | grep -Eq "${GIT}commit"; then
  if [ "$has_commits" = 1 ] && [ "$creates_branch" = 0 ] && { [ "$branch" = main ] || [ "$branch" = master ]; }; then
    block "Bloqueado: commit direto em '$branch'. Crie uma branch antes: git switch -c feat/<slug> (ou fix/, chore/, docs/, refactor/)."
  fi
fi

# push
if printf '%s' "$cmd" | grep -Eq "${GIT}push"; then
  if printf '%s' "$cmd" | grep -Eq -- '(--force([[:space:]]|$)|[[:space:]]-f([[:space:]]|$)|[[:space:]]\+[^[:space:]]+)'; then
    block "Bloqueado: force push. Se precisar reescrever a branch de feature, use --force-with-lease e confirme com o usuário."
  fi
  # Exceção: push inicial — a branch protegida ainda não existe no remoto (repo recém-criado / bootstrap).
  remote="$(printf '%s' "$cmd" | sed -E 's/.*push//' | tr ' ' '\n' | grep -Ev '^(-|$)' | head -n1)"
  remote="${remote:-origin}"
  initial_push=0
  if git -C "$cwd" remote get-url "$remote" >/dev/null 2>&1 \
    && [ -z "$(git -C "$cwd" ls-remote --heads "$remote" main master 2>/dev/null)" ]; then
    initial_push=1
  fi

  if [ "$initial_push" = 0 ] && printf '%s' "$cmd" | grep -Eq '([[:space:]]|:)(main|master)([[:space:]]|$)'; then
    block "Bloqueado: push direto para main/master. Abra um PR (/ship)."
  fi
  if [ "$initial_push" = 0 ] && [ "$creates_branch" = 0 ] && { [ "$branch" = main ] || [ "$branch" = master ]; } && [ "$has_commits" = 1 ]; then
    if ! printf '%s' "$cmd" | grep -Eq 'push[[:space:]].*[[:space:]](--tags|[^-[:space:]]+[[:space:]]+[^-[:space:]]+)'; then
      block "Bloqueado: você está em '$branch' e o push iria para ela. Crie uma branch e abra um PR."
    fi
  fi
fi

exit 0
