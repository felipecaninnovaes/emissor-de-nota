#!/usr/bin/env bash
# Funções compartilhadas pelos hooks do Claude Code.
# Uso: source "$(dirname "$0")/_lib.sh"; read_input; json_get .tool_name

HOOK_INPUT=""

read_input() {
  HOOK_INPUT="$(cat)"
}

# json_get <caminho.com.pontos>  →  valor (string crua) ou vazio.
# Usa jq; sem jq, cai para node ou python para funcionar no Windows sem dependências.
json_get() {
  local path="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$HOOK_INPUT" | jq -r "$path // empty"
  elif command -v node >/dev/null 2>&1; then
    printf '%s' "$HOOK_INPUT" | node -e '
      let d = ""; process.stdin.on("data", c => d += c).on("end", () => {
        let v = JSON.parse(d);
        for (const k of process.argv[1].split(".").filter(Boolean)) v = v == null ? undefined : v[k];
        if (v != null) process.stdout.write(typeof v === "string" ? v : JSON.stringify(v));
      });' "$path"
  elif command -v python >/dev/null 2>&1 || command -v python3 >/dev/null 2>&1; then
    local py; py="$(command -v python || command -v python3)"
    printf '%s' "$HOOK_INPUT" | "$py" -c '
import sys, json
v = json.load(sys.stdin)
for k in [p for p in sys.argv[1].split(".") if p]:
    v = v.get(k) if isinstance(v, dict) else None
if v is not None:
    sys.stdout.write(v if isinstance(v, str) else json.dumps(v))' "$path"
  else
    echo "hook: nem jq, node ou python encontrados; hook desativado" >&2
    exit 1 # erro não bloqueante: deny rules do settings.json continuam valendo
  fi
}

# block <mensagem>  →  bloqueia a ação; a mensagem volta para o Claude.
block() {
  echo "$1" >&2
  exit 2
}

project_dir() {
  printf '%s' "${CLAUDE_PROJECT_DIR:-$(json_get .cwd)}"
}
