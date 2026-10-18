-- Dados da obra (construção civil, regra E0370) da nota recorrente, em JSON (nfse_core::modelo::Obra).
-- NULL = o serviço não pede obra.
ALTER TABLE recorrencia ADD COLUMN obra_json TEXT;
