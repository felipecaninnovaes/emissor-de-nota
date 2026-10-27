-- Clientes do serviço, backups guardados no S3 e sessões do painel.

CREATE TABLE cliente (
    id          BIGSERIAL PRIMARY KEY,
    nome        TEXT NOT NULL,
    documento   TEXT NOT NULL DEFAULT '',
    email       TEXT NOT NULL DEFAULT '',
    observacao  TEXT NOT NULL DEFAULT '',
    ativo       BOOLEAN NOT NULL DEFAULT TRUE,
    -- SHA-256 do token; o token em claro só aparece uma vez, ao ser gerado.
    token_hash  TEXT UNIQUE,
    criado_em   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE backup (
    id          BIGSERIAL PRIMARY KEY,
    cliente_id  BIGINT NOT NULL REFERENCES cliente (id) ON DELETE CASCADE,
    chave       TEXT NOT NULL UNIQUE,
    nome        TEXT NOT NULL,
    tamanho     BIGINT NOT NULL,
    sha256      TEXT NOT NULL,
    criado_em   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX backup_por_cliente ON backup (cliente_id, criado_em DESC, id DESC);

CREATE TABLE sessao (
    id_hash     TEXT PRIMARY KEY,
    expira_em   TIMESTAMPTZ NOT NULL
);
