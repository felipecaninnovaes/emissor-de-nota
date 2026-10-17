-- Notas emitidas a partir de planilha (convênios). `chave` identifica a linha pelo conteúdo
-- (empresa, tomador, serviço, valor, descrição, mês, informações e a ocorrência na planilha):
-- importar a mesma planilha de novo não emite outra vez as linhas já autorizadas.

CREATE TABLE emissao_importada (
    id        INTEGER PRIMARY KEY,
    chave     TEXT NOT NULL,
    arquivo   TEXT,
    linha     INTEGER,
    dps_id    INTEGER NOT NULL UNIQUE REFERENCES dps (id),
    criado_em TEXT NOT NULL
);
CREATE INDEX emissao_importada_chave ON emissao_importada (chave);
