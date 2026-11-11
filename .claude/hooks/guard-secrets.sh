#!/usr/bin/env bash
# PreToolUse (Write|Edit|MultiEdit): impede gravar segredos em arquivos e escrever em .env reais.
# A leitura de .env/chaves é bloqueada pelas deny rules em .claude/settings.json.
source "$(dirname "$0")/_lib.sh"
read_input

file="$(json_get .tool_input.file_path)"
name="$(basename "${file//\\//}")"

case "$name" in
  .env.example | .env.sample | .env.template) ;;
  .env | .env.*)
    block "Bloqueado: não escreva em '$name'. Documente a variável em .env.example e peça ao usuário para preenchê-la localmente." ;;
esac

content="$(json_get .tool_input.content)$(json_get .tool_input.new_string)$(json_get .tool_input.edits)"
[ -z "$content" ] && exit 0

patterns=(
  'AKIA[0-9A-Z]{16}'                          # AWS access key
  '-----BEGIN ([A-Z]+ )?PRIVATE KEY-----'     # chave privada PEM
  'gh[pousr]_[A-Za-z0-9]{36,}'                # GitHub token
  'github_pat_[A-Za-z0-9_]{50,}'              # GitHub fine-grained PAT
  'sk-(ant-|proj-)?[A-Za-z0-9_-]{32,}'        # Anthropic / OpenAI
  'xox[baprs]-[A-Za-z0-9-]{10,}'              # Slack
  'AIza[0-9A-Za-z_-]{35}'                     # Google API key
  'glpat-[A-Za-z0-9_-]{20,}'                  # GitLab PAT
)

for p in "${patterns[@]}"; do
  if printf '%s' "$content" | grep -Eq -- "$p"; then
    block "Bloqueado: o conteúdo de '$name' parece conter um segredo (padrão: $p). Leia de uma variável de ambiente e documente em .env.example. Se for um valor fictício de teste, use um placeholder óbvio como 'test-token'."
  fi
done

exit 0
