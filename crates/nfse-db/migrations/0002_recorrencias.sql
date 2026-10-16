-- Notas recorrentes (mensalidades): modelo de nota emitido uma vez por mês de referência.

CREATE TABLE recorrencia (
    id                         INTEGER PRIMARY KEY,
    empresa_id                 INTEGER NOT NULL REFERENCES empresa (id),
    -- Tomador vinculado à empresa; NULL = nota sem tomador identificado.
    pessoa_id                  INTEGER REFERENCES pessoa (id),
    servico_id                 INTEGER NOT NULL REFERENCES servico (id),
    -- NULL = usa o valor sugerido / a descrição padrão do serviço.
    valor                      INTEGER,
    descricao                  TEXT,
    informacoes_complementares TEXT,
    ativa                      INTEGER NOT NULL DEFAULT 1,
    criado_em                  TEXT NOT NULL
);

-- Cada tentativa de emitir uma recorrência num mês, gravada ao reservar a DPS (antes do envio):
-- o mês só fica livre de novo se a DPS for rejeitada ou a nota cancelada.
CREATE TABLE emissao_recorrente (
    id             INTEGER PRIMARY KEY,
    recorrencia_id INTEGER NOT NULL REFERENCES recorrencia (id),
    -- AAAA-MM
    mes_referencia TEXT NOT NULL,
    dps_id         INTEGER NOT NULL UNIQUE REFERENCES dps (id)
);
CREATE INDEX emissao_recorrente_mes ON emissao_recorrente (recorrencia_id, mes_referencia);
