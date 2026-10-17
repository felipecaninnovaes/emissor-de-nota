-- Listagem de notas por empresa, da mais recente para a mais antiga, sem ordenar em memória.
CREATE INDEX dps_empresa_id ON dps (empresa_id, id DESC);
