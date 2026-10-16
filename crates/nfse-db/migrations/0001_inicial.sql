-- Esquema inicial. SQL compatível com SQLite; datas em texto ISO 8601 (UTC salvo indicação).

CREATE TABLE configuracao (
    chave TEXT PRIMARY KEY,
    valor TEXT NOT NULL
);

-- Dado público e único por documento: prestadores e tomadores.
CREATE TABLE pessoa (
    id            INTEGER PRIMARY KEY,
    tipo          TEXT NOT NULL CHECK (tipo IN ('cnpj', 'cpf')),
    documento     TEXT NOT NULL UNIQUE,
    nome          TEXT NOT NULL,
    inscricao_municipal TEXT,
    logradouro    TEXT,
    numero        TEXT,
    complemento   TEXT,
    bairro        TEXT,
    cep           TEXT,
    municipio     TEXT,
    telefone      TEXT,
    email         TEXT,
    -- De onde vieram os dados (digitação, BrasilAPI...) e quando.
    fonte         TEXT NOT NULL DEFAULT 'manual',
    atualizado_em TEXT NOT NULL
);

CREATE TABLE empresa (
    id                     INTEGER PRIMARY KEY,
    pessoa_id              INTEGER NOT NULL UNIQUE REFERENCES pessoa (id),
    inscricao_municipal    TEXT,
    -- A IM só vai na DPS se o CNC do ambiente tiver informações complementares (E0120).
    im_na_dps_producao     INTEGER NOT NULL DEFAULT 1,
    im_na_dps_homologacao  INTEGER NOT NULL DEFAULT 0,
    municipio_emissor      TEXT NOT NULL,
    regime_json            TEXT NOT NULL,
    ambiente               TEXT NOT NULL CHECK (ambiente IN ('producao', 'homologacao')),
    serie                  TEXT NOT NULL,
    ativa                  INTEGER NOT NULL DEFAULT 1,
    criado_em              TEXT NOT NULL
);

-- Próximo número por empresa, ambiente e série (a série 70000 é do Emissor Web).
CREATE TABLE numeracao (
    empresa_id     INTEGER NOT NULL REFERENCES empresa (id),
    ambiente       TEXT NOT NULL,
    serie          TEXT NOT NULL,
    proximo_numero INTEGER NOT NULL CHECK (proximo_numero >= 1),
    PRIMARY KEY (empresa_id, ambiente, serie)
);

-- .pfx e senha cifrados com a chave do cofre; histórico mantido ao trocar de certificado.
CREATE TABLE certificado (
    id            INTEGER PRIMARY KEY,
    empresa_id    INTEGER NOT NULL REFERENCES empresa (id),
    pfx_cifrado   BLOB NOT NULL,
    senha_cifrada BLOB NOT NULL,
    thumbprint    TEXT NOT NULL,
    titular       TEXT NOT NULL,
    emissor       TEXT NOT NULL,
    valido_de     TEXT NOT NULL,
    valido_ate    TEXT NOT NULL,
    ativo         INTEGER NOT NULL DEFAULT 1,
    importado_em  TEXT NOT NULL,
    UNIQUE (empresa_id, thumbprint)
);
CREATE UNIQUE INDEX certificado_um_ativo ON certificado (empresa_id) WHERE ativo = 1;

-- Vínculo reutilizável: o mesmo tomador (pessoa) atende várias empresas.
CREATE TABLE empresa_tomador (
    empresa_id INTEGER NOT NULL REFERENCES empresa (id),
    pessoa_id  INTEGER NOT NULL REFERENCES pessoa (id),
    emails     TEXT,
    observacao TEXT,
    PRIMARY KEY (empresa_id, pessoa_id)
);

CREATE TABLE servico (
    id                  INTEGER PRIMARY KEY,
    empresa_id          INTEGER NOT NULL REFERENCES empresa (id),
    nome                TEXT NOT NULL,
    -- Descrição padrão; aceita a variável {mes_ref}.
    descricao           TEXT NOT NULL,
    codigo_trib_nac     TEXT NOT NULL,
    codigo_trib_mun     TEXT,
    nbs                 TEXT,
    -- Issqn, TributosFederais, TotalTributos e IbsCbs do modelo da DPS, em JSON.
    tributacao_json     TEXT NOT NULL,
    valor_sugerido      INTEGER,
    ativo               INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE dps (
    id             INTEGER PRIMARY KEY,
    empresa_id     INTEGER NOT NULL REFERENCES empresa (id),
    ambiente       TEXT NOT NULL,
    serie          TEXT NOT NULL,
    numero         INTEGER NOT NULL,
    id_dps         TEXT NOT NULL UNIQUE,
    situacao       TEXT NOT NULL CHECK (situacao IN
                     ('reservada', 'enviando', 'autorizada', 'rejeitada', 'incerta', 'cancelada', 'substituida')),
    versao_leiaute TEXT NOT NULL,
    modelo_json    TEXT,
    xml_assinado   TEXT,
    tentativas     INTEGER NOT NULL DEFAULT 0,
    ultimo_erro    TEXT,
    criado_em      TEXT NOT NULL,
    atualizado_em  TEXT NOT NULL,
    UNIQUE (empresa_id, ambiente, serie, numero)
);
CREATE INDEX dps_situacao ON dps (situacao);

CREATE TABLE nfse (
    id            INTEGER PRIMARY KEY,
    dps_id        INTEGER NOT NULL UNIQUE REFERENCES dps (id),
    chave         TEXT NOT NULL UNIQUE,
    numero        TEXT,
    -- XML autorizado exatamente como a Sefin devolveu (documento fiscal) e seu SHA-256.
    xml           TEXT NOT NULL,
    xml_sha256    TEXT NOT NULL,
    valor_servico INTEGER NOT NULL,
    competencia   TEXT NOT NULL,
    autorizada_em TEXT NOT NULL
);

CREATE TABLE evento (
    id         INTEGER PRIMARY KEY,
    nfse_id    INTEGER NOT NULL REFERENCES nfse (id),
    tipo       TEXT NOT NULL,
    xml        TEXT NOT NULL,
    criado_em  TEXT NOT NULL
);

CREATE TABLE auditoria (
    id         INTEGER PRIMARY KEY,
    quando     TEXT NOT NULL,
    acao       TEXT NOT NULL,
    empresa_id INTEGER,
    detalhe    TEXT
);
