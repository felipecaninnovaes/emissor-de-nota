-- Ajuste aplicado por `ajustes::dps_unica_por_ambiente` (fora do sqlx, que sempre abre
-- transação e não deixa desligar as chaves estrangeiras, necessário para recriar `dps`).
-- O Id da DPS (município + CNPJ + série + número) só é único dentro de um ambiente: a Sefin de
-- Produção e a de Produção Restrita são separadas, e a numeração de cada ambiente começa do 1.

CREATE TABLE dps_nova (
    id             INTEGER PRIMARY KEY,
    empresa_id     INTEGER NOT NULL REFERENCES empresa (id),
    ambiente       TEXT NOT NULL,
    serie          TEXT NOT NULL,
    numero         INTEGER NOT NULL,
    id_dps         TEXT NOT NULL,
    situacao       TEXT NOT NULL CHECK (situacao IN
                     ('reservada', 'enviando', 'autorizada', 'rejeitada', 'incerta', 'cancelada', 'substituida')),
    versao_leiaute TEXT NOT NULL,
    modelo_json    TEXT,
    xml_assinado   TEXT,
    tentativas     INTEGER NOT NULL DEFAULT 0,
    ultimo_erro    TEXT,
    criado_em      TEXT NOT NULL,
    atualizado_em  TEXT NOT NULL,
    UNIQUE (empresa_id, ambiente, serie, numero),
    UNIQUE (ambiente, id_dps)
);

INSERT INTO dps_nova (id, empresa_id, ambiente, serie, numero, id_dps, situacao, versao_leiaute, modelo_json,
                      xml_assinado, tentativas, ultimo_erro, criado_em, atualizado_em)
SELECT id, empresa_id, ambiente, serie, numero, id_dps, situacao, versao_leiaute, modelo_json,
       xml_assinado, tentativas, ultimo_erro, criado_em, atualizado_em
FROM dps;

DROP TABLE dps;
ALTER TABLE dps_nova RENAME TO dps;
CREATE INDEX dps_situacao ON dps (situacao);
