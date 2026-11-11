#!/usr/bin/env bash
# Stop: impede o Claude de encerrar o turno com `just check` quebrado.
# Só roda se há mudanças fora de docs (*.md) e se o estado mudou desde o último check verde.
source "$(dirname "$0")/_lib.sh"
read_input

dir="$(project_dir)"
cd "$dir" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

changed="$(git status --porcelain=v1 -uall | sed -E 's/^.{3}//; s/.* -> //' | grep -Ev '\.md$' || true)"
[ -z "$changed" ] && exit 0

# Fingerprint do working tree: evita rodar o check de novo se nada mudou desde o último sucesso.
state="$({ git status --porcelain=v1 -uall; git diff; git diff --cached; \
  git ls-files -o --exclude-standard -z | xargs -0 -r git hash-object --; } | git hash-object --stdin)"
stamp="$dir/.claude/.stop-check.ok"
[ -f "$stamp" ] && [ "$(cat "$stamp")" = "$state" ] && exit 0

if ! command -v just >/dev/null 2>&1; then
  echo "stop-check: 'just' não encontrado — verificação NÃO executada. Entre no devbox shell ou instale o just." >&2
  exit 1
fi

if out="$(just check 2>&1)"; then
  printf '%s' "$state" > "$stamp"
  exit 0
fi

{
  echo "'just check' falhou. Corrija antes de encerrar (últimas linhas):"
  printf '%s\n' "$out" | tail -n 60
} >&2
exit 2
