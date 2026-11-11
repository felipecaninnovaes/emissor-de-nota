# Presets de stack para o justfile

Ponto de partida — confirme com o usuário e ajuste ao que o projeto já usa. Prefira ferramentas que formatam **um arquivo isolado** (para `fmt-file`) e que aceitam **filtro de teste** (para `test *args`).

| Receita | TypeScript (pnpm) | Python (uv) | Go | Rust | .NET | Java/Kotlin (Gradle) |
|---|---|---|---|---|---|---|
| devbox | `nodejs@22 pnpm@10` | `python@3.13 uv@latest` | `go@1.24 golangci-lint@latest` | `rustup@latest` | `dotnet-sdk_9` | `jdk21 gradle` |
| setup | `pnpm install --frozen-lockfile` | `uv sync --frozen` | `go mod download` | `cargo fetch` | `dotnet restore` | `./gradlew dependencies` |
| fmt | `pnpm biome format --write .` | `uv run ruff format .` | `gofmt -w .` | `cargo fmt` | `dotnet format` | `./gradlew spotlessApply` |
| fmt-check | `pnpm biome format .` | `uv run ruff format --check .` | `test -z "$(gofmt -l .)"` | `cargo fmt --check` | `dotnet format --verify-no-changes` | `./gradlew spotlessCheck` |
| fmt-file | `pnpm biome format --write {{file}}` | `uv run ruff format {{file}}` | `gofmt -w {{file}}` | `rustfmt {{file}}` | `dotnet format --include {{file}}` | `./gradlew spotlessApply` |
| lint | `pnpm biome lint . && pnpm tsc --noEmit` | `uv run ruff check . && uv run pyright` | `go vet ./... && golangci-lint run` | `cargo clippy --all-targets -- -D warnings` | `dotnet build -warnaserror` | `./gradlew check -x test` |
| test | `pnpm vitest run {{args}}` | `uv run pytest {{args}}` | `go test ./... {{args}}` | `cargo test {{args}}` | `dotnet test {{args}}` | `./gradlew test {{args}}` |
| build | `pnpm build` | `uv build` | `go build ./...` | `cargo build --release` | `dotnet build -c Release` | `./gradlew build` |
| dependabot | `npm` | `uv` (ou `pip`) | `gomod` | `cargo` | `nuget` | `gradle` |

## Cuidados
- **fmt-file** recebe qualquer arquivo editado (inclusive `.md`, `.json`). O comando deve ignorar extensões que não conhece ou a receita deve filtrar a extensão — um formatter que falha em arquivos desconhecidos gera ruído a cada edição.
- **Windows:** as receitas rodam em PowerShell (`set windows-shell`). Evite sintaxe só-bash (`$(...)`, `&&` em PowerShell 5) ou use `[unix]`/`[windows]` em receitas separadas quando necessário. No devbox/Linux rodam em `sh`.
- **Monorepo:** `test *args` deve aceitar o caminho de um pacote; documente em CLAUDE.md como rodar o check de um pacote só.
- Em Go, `gofmt -l` dentro de `test -z` é bash; para Windows crie uma variante `[windows]`.
