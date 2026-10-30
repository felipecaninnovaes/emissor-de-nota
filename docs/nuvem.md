# Servidor do backup na nuvem

Como subir e operar o servidor que guarda os backups enviados pelo aplicativo ([ADR 0017](decisoes/0017-backup-na-nuvem.md)). O código fica em [`servidor/`](../servidor/); a imagem do painel é publicada em `ghcr.io/felipecaninnovaes/emissor-nuvem`.

## O que sobe

| Serviço | Imagem | Papel |
| --- | --- | --- |
| `painel` | `ghcr.io/felipecaninnovaes/emissor-nuvem` | Painel do administrador e API do aplicativo (porta 8080, só em `127.0.0.1`) |
| `banco` | `postgres:17` | Clientes, lista dos backups e sessões do painel |
| `s3` | `versity/versitygw` | Arquivos dos backups, gravados como arquivos comuns; sem porta publicada |
| `caddy` (opcional) | `caddy:2` | HTTPS com certificado automático, para VPS sem proxy reverso |

O aplicativo só fala com o painel. O backup chega já cifrado com a senha mestra do escritório: o servidor não consegue abrir o conteúdo.

## Instalar na VPS

1. Copie para uma pasta da VPS os arquivos `servidor/compose.yaml`, `servidor/Caddyfile` e `servidor/.env.exemplo`.
2. `cp .env.exemplo .env` e preencha. Para os segredos (`ADMIN_SENHA`, `POSTGRES_SENHA`, `S3_CHAVE`, `S3_SEGREDO`), use `openssl rand -hex 32`.
3. Aponte o DNS do domínio (`DOMINIO`) para a VPS.
4. Suba:
   - sem proxy reverso na VPS: `docker compose --profile https up -d` (o Caddy pede o certificado sozinho; portas 80 e 443 livres);
   - com proxy próprio (Nginx, Traefik, Caddy já instalado): `docker compose up -d` e encaminhe o domínio para `127.0.0.1:8080` (ou a `PORTA_PAINEL`), sem limite de tamanho do corpo da requisição menor que o `TAMANHO_MAXIMO_MB`.
5. Abra `https://<DOMINIO>` e entre com `ADMIN_USUARIO` e `ADMIN_SENHA`.

Se o proxy não repassa o cabeçalho `Host` original, preencha `URL_PUBLICA`; sem ela, os formulários do painel são recusados como "de outra origem".

## Variáveis (`.env`)

| Variável | Padrão | Para quê |
| --- | --- | --- |
| `DOMINIO` | | Domínio do painel (Caddy) |
| `URL_PUBLICA` | | Endereço público do painel, para conferir a origem dos formulários |
| `ADMIN_USUARIO`, `ADMIN_SENHA` | `admin`, — | Login do painel; senha com 12 caracteres ou mais |
| `POSTGRES_SENHA` | — | Senha do Postgres do painel |
| `S3_CHAVE`, `S3_SEGREDO` | — | Credenciais internas do S3 (painel ↔ versitygw) |
| `MANTER_BACKUPS` | 30 | Backups guardados por cliente; os mais antigos são apagados a cada envio |
| `TAMANHO_MAXIMO_MB` | 500 | Tamanho máximo de um backup |
| `VERSAO_PAINEL` | `latest` | Tag da imagem do painel |
| `PORTA_PAINEL` | 8080 | Porta do painel em `127.0.0.1` |
| `COOKIE_SEGURO` | `true` | `false` só para testar em `http://localhost` |

## Clientes

- **Novo cliente**: nome (CNPJ/CPF, e-mail e observação são opcionais). Ao salvar, o painel mostra o token **uma vez**. Envie ao cliente o endereço do painel e o token; ele cola os dois no aplicativo em **Configurações → Backup na nuvem**.
- **Desativar** (atraso ou cancelamento): o aplicativo deixa de enviar backups novos e mostra "assinatura inativa". Os backups já guardados continuam disponíveis para restaurar. **Ativar** libera de novo.
- **Gerar novo token**: o anterior para de valer na hora (token vazado ou troca de máquina).
- **Excluir cliente e backups**: apaga os arquivos e o cadastro; pede o nome do cliente como confirmação. Não tem volta.
- Na página do cliente dá para baixar ou excluir um backup específico.

## Dados e backup da VPS

Tudo fica em `./dados`, ao lado do `compose.yaml`:

- `dados/s3/backups/clientes/<id>/*.nfbk`: os backups, como arquivos comuns;
- `dados/postgres`: o banco do painel (cadastro dos clientes e a lista dos backups);
- `dados/caddy`: certificados do Caddy.

O backup diário da VPS precisa levar essa pasta. A cópia do Postgres em arquivo só é consistente se o backup da VPS for um snapshot do disco; se for cópia de arquivos, acrescente um `pg_dump` diário:

```bash
docker compose exec -T banco pg_dump -U nuvem nuvem > dados/painel.sql
```

## Atualizar

```bash
docker compose pull painel
docker compose up -d painel
```

As migrações do banco do painel rodam sozinhas na subida. Para fixar uma versão, use `VERSAO_PAINEL=X.Y.Z` no `.env`.

## API do aplicativo

Prefixo `/api/v1`, cabeçalho `Authorization: Bearer <token>`. Erros sempre como `{ "erro": "texto" }`; token ausente ou inválido dá `401`.

| Método e caminho | Resposta |
| --- | --- |
| `GET /situacao` | `{ cliente, ativo, mantidos, tamanho_maximo }` |
| `POST /backups` (corpo = `.nfbk`; `Content-Length`, `X-Sha256`, `X-Nome-Arquivo`) | `201 { id, nome, tamanho, criado_em }`; `403` cliente inativo; `413` acima do limite; `422` SHA-256 não confere |
| `GET /backups` | `[{ id, nome, tamanho, sha256, criado_em }]`, mais recente primeiro |
| `GET /backups/{id}` | O arquivo, com `X-Sha256`; funciona também com o cliente inativo |

`GET /saude` responde `ok` (usado pelo `HEALTHCHECK` da imagem).

## Desenvolvimento

```bash
cd servidor
cargo test                                  # testes sem banco
DATABASE_URL=postgres://... cargo test -- --include-ignored   # API e painel contra um Postgres
```

Com o Devbox, `devbox run up` sobe esse Postgres (`compose.yaml` da raiz) e `devbox run test` já roda os testes do servidor contra ele ([desenvolvimento.md](desenvolvimento.md#ambiente-com-devbox-linux-e-macos)).

O CI roda fmt, clippy e todos os testes com um Postgres de serviço (job `servidor`) e confere as licenças (job `licencas-servidor`). Para publicar uma versão fixa da imagem: suba `version` em `servidor/Cargo.toml`, faça commit e crie a tag `nuvem-vX.Y.Z`.
