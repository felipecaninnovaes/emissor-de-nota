-- Cada envio (ou tentativa) da NFS-e por e-mail ao tomador.
CREATE TABLE envio_email (
    id        INTEGER PRIMARY KEY,
    nfse_id   INTEGER NOT NULL REFERENCES nfse (id),
    -- Endereços separados por "; ".
    para      TEXT NOT NULL,
    assunto   TEXT NOT NULL,
    -- NULL = enviado; senão, o motivo da falha.
    erro      TEXT,
    criado_em TEXT NOT NULL
);
CREATE INDEX envio_email_nfse ON envio_email (nfse_id);
