# Esquemas XSD da NFS-e Nacional

Cópias dos pacotes oficiais publicados no [Portal NFS-e](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica),
sem alteração. Cada pasta é uma versão; a DPS gerada guarda a versão do leiaute para reprocessar notas antigas.

| Pasta | Origem | Observação |
| --- | --- | --- |
| `v1.01-prodrest-20261020` | Produção Restrita, `esquemas_prod_restrita_20261020.zip` | NT 009: `finNFSe` em `infDPS`, `vAjusteBC`; valida o leiaute NT 009 |
| `v1.01-prodrest-20260727` | Produção Restrita, `esquemas-nfse-rtc-v1-01-20260727.zip` | CNPJ alfanumérico; valida o leiaute NT 004 |

O pacote de Produção `nfse-esquemas_xsd-v1-01-20260209.zip` não foi incluído: o padrão de `TSSerieDPS`
usa `^...$`, que em XSD são caracteres literais, e recusa qualquer série (inclusive as de notas autorizadas).
