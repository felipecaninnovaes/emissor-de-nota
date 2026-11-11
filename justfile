# Contrato de comandos do projeto.
# Claude Code, hooks e pre-commit chamam SOMENTE estas receitas:
# troque a implementação, nunca o nome.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Lista as receitas
default:
    @just --list --unsorted

# Instala os git hooks e baixa as dependências
setup:
    pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg
    cargo fetch --locked

# Verificação completa do workspace (exigida pelo hook Stop; igual ao job Windows do CI)
check: fmt-check lint _test-workspace

# Formata todo o código
fmt:
    cargo fmt --all

# Falha se algum arquivo não estiver formatado
fmt-check:
    cargo fmt --all --check

# Formata um único arquivo .rs; outras extensões são ignoradas (chamado pelo hook PostToolUse)
[windows]
fmt-file file:
    if ('{{file}}' -like '*.rs') { rustfmt --edition 2024 '{{file}}' }

# Formata um único arquivo .rs; outras extensões são ignoradas (chamado pelo hook PostToolUse)
[unix]
fmt-file file:
    case '{{file}}' in *.rs) rustfmt --edition 2024 '{{file}}' ;; esac

# Clippy em todo o workspace, avisos viram erro
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Testes do workspace; aceita filtro e opções do cargo: just test -p nfse-xml, just test nome_do_teste
test *args:
    cargo test --locked {{args}}

# Testes do workspace para o check
[windows]
_test-workspace:
    cargo test --workspace --locked

# Testes do workspace para o check; identidade_mtls_a_partir_do_pfx só passa no Windows (docs/desenvolvimento.md#problemas-conhecidos)
[unix]
_test-workspace:
    cargo test --workspace --locked -- --skip identidade_mtls_a_partir_do_pfx

# Build de release do workspace
build:
    cargo build --workspace --release --locked

# Roda o aplicativo
run:
    cargo run -p emissor-nfse

# fmt, clippy e testes do servidor do backup; precisa do Postgres do compose.yaml da raiz (docker compose up -d --wait)
check-servidor $DATABASE_URL="postgres://nuvem:nuvem@127.0.0.1:55432/nuvem":
    cargo fmt --manifest-path servidor/Cargo.toml --check
    cargo clippy --manifest-path servidor/Cargo.toml --all-targets --locked -- -D warnings
    cargo test --manifest-path servidor/Cargo.toml --locked -- --include-ignored

# Procura segredos no histórico git
scan-secrets:
    gitleaks git --redact --no-banner .

# Roda todos os hooks do pre-commit em todos os arquivos
hooks-run:
    pre-commit run --all-files --show-diff-on-failure

# Atualiza as versões dos hooks do pre-commit
hooks-update:
    pre-commit autoupdate
