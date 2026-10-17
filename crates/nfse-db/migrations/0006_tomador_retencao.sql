-- O tomador decide se retém: "Retém impostos federais" e "Retém ISS" no vínculo com a empresa.
-- O serviço diz quais retenções valem (sempre ou só para tomador que retém).
ALTER TABLE empresa_tomador ADD COLUMN retem_federais INTEGER NOT NULL DEFAULT 0;
ALTER TABLE empresa_tomador ADD COLUMN retem_iss INTEGER NOT NULL DEFAULT 0;
