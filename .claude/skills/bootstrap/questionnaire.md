# Questionário de bootstrap

Rodadas de até 4 perguntas (AskUserQuestion). Pule o que o diagnóstico já respondeu; ofereça o inferido como opção recomendada.

## Rodada 1 — Identidade
1. **Nome e descrição** do projeto (1–2 frases: o que é, para quem, o que não é).
2. **Tipo**: biblioteca · CLI · API/serviço · aplicação web · app mobile · monorepo (quais pacotes?).
3. **GitHub**: `owner/repo` (para CODEOWNERS, links, issue templates).
4. **Titular da licença MIT** e **contato** para segurança/conduta (e-mail ou "usar GitHub private vulnerability reporting").

## Rodada 2 — Stack
5. **Linguagem e versão do runtime** (será fixada no devbox).
6. **Gerenciador de pacotes / build** (pnpm, npm, uv, poetry, cargo, go, gradle, maven, dotnet…).
7. **Framework principal**, se houver.
8. **Monorepo?** ferramenta (pnpm workspaces, turborepo, nx, cargo workspace, go work, uv workspace) ou repositório único.

## Rodada 3 — Contrato de qualidade (preencha com o preset da stack e confirme)
9. **Formatter** (e se ele formata um arquivo isolado → `fmt-file`).
10. **Linter + typecheck**.
11. **Test runner** e como rodar **um único teste** (`just test <x>`).
12. **Cobertura mínima** no CI? (nenhuma · reportar sem falhar · falhar abaixo de N%).

## Rodada 4 — Arquitetura e ambiente
13. **Estilo arquitetural**: camadas · hexagonal/ports & adapters · por feature (vertical slices) · decidir depois.
14. **Integrações externas** (banco, filas, APIs de terceiros) → serviços no CI e variáveis em `.env.example`.
15. **Serviços locais**: nenhum · `docker compose` · `devbox services` (process-compose).
16. **Variáveis de ambiente** necessárias desde já.

## Rodada 5 — Entrega
17. **Distribuição**: nenhuma ainda · pacote em registry (npm/PyPI/crates/…) · imagem de container · deploy em plataforma (qual?).
18. **Versionamento/release**: SemVer manual com `gh release` · release-please (automático a partir dos Conventional Commits) · nenhum por enquanto.
19. **Esqueleto mínimo** com um teste real agora? (recomendado: sim).
20. **Configurar o GitHub agora** (repo, rulesets, segurança)? sim · depois.
