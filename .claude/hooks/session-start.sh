#!/usr/bin/env bash
# SessionStart: avisa (no contexto do Claude) sobre ferramentas ausentes e bootstrap pendente.
dir="${CLAUDE_PROJECT_DIR:-.}"
missing=()
for t in just git gitleaks pre-commit; do
  command -v "$t" >/dev/null 2>&1 || missing+=("$t")
done
command -v jq >/dev/null 2>&1 || command -v node >/dev/null 2>&1 || command -v python >/dev/null 2>&1 \
  || missing+=("jq")

if [ ${#missing[@]} -gt 0 ]; then
  echo "Ambiente: ferramentas ausentes: ${missing[*]}. Rode dentro do 'devbox shell' (ou instale-as); hooks e 'just check' podem estar degradados."
fi

if grep -q 'TODO(bootstrap)' "$dir/justfile" 2>/dev/null; then
  echo "Projeto ainda não inicializado a partir do modelo: sugira ao usuário rodar /bootstrap antes de implementar."
fi
exit 0
