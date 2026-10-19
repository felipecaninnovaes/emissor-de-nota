-- Tipo do tomador no vínculo com a empresa: decide quais retenções federais se aplicam.
-- privado e federal: as do serviço; estadual_municipal: só IRRF e INSS (não retém PIS, COFINS nem CSLL).
ALTER TABLE empresa_tomador ADD COLUMN tipo TEXT NOT NULL DEFAULT 'privado';
