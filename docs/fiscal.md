# Regras fiscais e integração com a Sefin Nacional

Referência para quem mexe na emissão: o que a API Sefin Nacional / ADN exige e como o sistema se comporta. O fluxo interno está em [arquitetura.md](arquitetura.md). O que foi confirmado em Produção está marcado como tal; o resto vem do Anexo I (leiaute v1.01-20260209).

## Ambientes

- Produção: `https://sefin.nfse.gov.br/SefinNacional`. Produção Restrita: `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` (`crates/nfse-nacional/src/lib.rs`).
- O ambiente é escolhido por empresa. Cancelamento e reconciliação usam o ambiente gravado na DPS, não o atual da empresa.
- Na Produção Restrita o contribuinte precisa estar no cadastro nacional desse ambiente; sem isso a Sefin rejeita com E0084. Por isso os testes reais deste projeto foram feitos em Produção.

## Comunicação

- **mTLS** com o e-CNPJ A1 do prestador (EKU "Client Authentication"), via `reqwest` com `native-tls`.
- **Transporte**: corpo JSON com o XML em GZip + Base64 (`dpsXmlGZipB64`, `pedidoRegistroEventoXmlGZipB64`); função `gzip_base64`.
- **Operações**:

| Operação | Endpoint | Situação |
| --- | --- | --- |
| Emitir | `POST /nfse` | feito |
| Consultar NFS-e | `GET /nfse/{chaveAcesso}` | feito (reconciliação) |
| Consultar DPS | `GET /dps/{id}` | feito (reconciliação e "Consultar na Sefin") |
| Registrar evento | `POST /nfse/{chave}/eventos` | feito para cancelamento (e101101) |
| Consultar eventos, distribuição DF-e, parâmetros municipais | — | não implementado |

## Assinatura XMLDSIG

Implementação própria em `crates/nfse-xml/src/assinatura.rs` e `c14n.rs`, conferida contra `xmlsec1` nos testes.

- **DPS e eventos que enviamos**: C14N inclusiva, RSA-SHA1, só o certificado final no `KeyInfo`, **sem prefixo `ds:`** (prefixo gera E1228). Confirmado em Produção.
- **NFS-e devolvida pela Sefin**: C14N exclusiva, RSA-SHA256. O sistema confere a assinatura antes de arquivar, mas só registra aviso se não conferir ([ADR 0007](decisoes/0007-assinatura-da-sefin-como-aviso.md)). A cadeia ICP-Brasil não é validada.

## Leiaute e XSD

- A ordem dos elementos segue o `xs:sequence` do XSD. A DPS usa o leiaute 1.01, gravado em cada DPS.
- Validamos com o XSD de **Produção Restrita** (`xsd/v1.01-prodrest-20260727`): o de Produção tem o padrão da série escrito com `^...$`, que em XSD são literais, e recusa qualquer série ([xsd/README.md](../xsd/README.md), [ADR 0005](decisoes/0005-xsd-de-producao-restrita.md)).
- A validação roda em testes marcados `#[ignore]` que exigem `xmllint` e `xmlsec1` (rodam no CI).

## Id da DPS e numeração

- Id: `DPS` + município IBGE + tipo de inscrição + CNPJ + série + número, 45 caracteres. É a chave de idempotência e é **único por ambiente** (a numeração de cada ambiente começa do 1).
- Série própria (padrão `1`). Pelo Anexo I, 00001–49999 é a faixa de aplicativo próprio; 70000–79999 é do Emissor Web nacional.
- O Anexo I não exige `nDPS` sem buracos nem prevê inutilização. A única regra do número é **E0014**: série e número já usados por uma DPS que gerou NFS-e não podem ser reusados. Como a série `1` pode ter sido usada por outro emissor, o botão "Consultar na Sefin" (`ajustar_numeracao_pela_sefin`) acha o primeiro número livre por busca exponencial e binária.
- Nota rejeitada é refeita com o **próximo** número ("Corrigir e emitir"). Ver [ADR 0003](decisoes/0003-serie-propria-e-numeracao.md).

## Regras de preenchimento

- **Competência** (`dCompet`) = data da emissão; o mês de referência vai só na descrição ([ADR 0004](decisoes/0004-competencia-igual-a-emissao.md)).
- **Total de tributos** (Anexo I): ME/EPP informa o % do Simples (`pTotTribSN`, E0712); não optante informa percentuais ou valores federal/estadual/municipal (E0713); MEI não usa `pTotTribSN` (E0710). Validado em `nfse-core` antes de gastar número.
- **Inscrição municipal** na DPS é configurável por ambiente (E0120 quando o cadastro nacional não a tem; E0116 quando o município a exige). Ver [ADR 0011](decisoes/0011-inscricao-municipal-por-ambiente.md).
- **Retenções do serviço** são percentuais (IRRF, CSLL, INSS, PIS, COFINS) e os valores são calculados sobre o valor de cada nota. PIS/COFINS retidos são somados em `vRetCSLL` (NT 007/2026). O serviço define quais retenções valem (sempre, ou só para tomador que retém); o tomador define se retém, por empresa. Ver [ADR 0012](decisoes/0012-retencoes-em-percentual.md). A revisão só **avisa** (não bloqueia) de retenção federal para optante do Simples e de PIS/COFINS/CSLL em nota de até R$ 215,05; o sistema não valida se a retenção é cabível para o item da LC 116.
- **Obra**: grupo de obra na DPS (regra E0370). **IBS/CBS**: grupo `IBSCBS` no cadastro de serviço (CST, cClassTrib, cIndOp).
- **Cancelamento**: motivos `erro-na-emissao`, `servico-nao-prestado`, `outros`; justificativa de 15 a 255 caracteres. Prazos variam por município. O PDF é regerado com marca d'água.
- **DANFSe**: PDF gerado localmente a partir do XML autorizado (layout v2.0, NT 008/2026), com QR Code da consulta pública, logomarca oficial e marca d'água de cancelada. Fontes procuradas no sistema (`nfse-pdf/src/fontes.rs`).

## Falhas e contingência

- Não há contingência offline. Com a Sefin fora, a nota fica `incerta` e é reconciliada ao abrir o sistema ou pelo botão "Verificar".
- **Limite de requisições (429)**: o lote espera e tenta mais uma vez; a tentativa recusada por limite não gera nota.
- Códigos de rejeição com orientação em português (`crates/nfse-nacional/src/mensagens.rs`): E0014, E0084, E0116, E0120, E0312, E0710, E0712, E0713, E0714, E0717, E1228, E1235, E2400, E2404.
- E0312: código de serviço não habilitado para a empresa no município.
- E0717 foi relatado por terceiros com alguns certificados "RFB e-CNPJ A1" em eventos de manifestação (que este sistema não envia). `nfse-poc cert-info` faz o diagnóstico do certificado.

## Fontes

- [Portal NFS-e — documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica)
- [Anexo I — leiaute e regras de negócio, v1.01-20260209](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual)
- [NT 007 SE/CGNFS-e](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf)
- [Requisitos da API Sefin — NotaGateway](https://notagateway.com.br/?p=6164)
- [Eventos da NFS-e — Buscador NCM](https://buscadorncm.com.br/nfse/eventos)
