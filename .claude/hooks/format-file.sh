#!/usr/bin/env bash
# PostToolUse (Write|Edit|MultiEdit): formata o arquivo editado via `just fmt-file`.
# Não bloqueia: falha de formatação aparece como aviso e o `just check` pega depois.
source "$(dirname "$0")/_lib.sh"
read_input

file="$(json_get .tool_input.file_path)"
[ -z "$file" ] || [ ! -f "$file" ] && exit 0
command -v just >/dev/null 2>&1 || exit 0

dir="$(project_dir)"
just --justfile "$dir/justfile" --working-directory "$dir" fmt-file "$file" >/dev/null 2>&1 \
  || echo "format-file: 'just fmt-file' falhou para $file" >&2
exit 0
