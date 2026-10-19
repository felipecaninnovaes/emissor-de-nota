-- Empresa importada de outra máquina (ADR 0015): a numeração precisa ser conferida na Sefin antes
-- da primeira emissão. 1 = a reserva de número fica bloqueada até "Consultar na Sefin".
ALTER TABLE empresa ADD COLUMN conferir_numeracao INTEGER NOT NULL DEFAULT 0;
