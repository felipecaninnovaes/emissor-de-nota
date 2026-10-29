# Regras fiscais e integração com a Sefin Nacional

Referência para quem mexe na emissão: o que a API Sefin Nacional / ADN exige e como o sistema se comporta. O fluxo interno está em [arquitetura.md](arquitetura.md). O que foi confirmado em Produção está marcado como tal; o resto vem do Anexo I (leiaute v1.01-20260209).

## Ambientes

- Produção: `https://sefin.nfse.gov.br/SefinNacional`. Produção Restrita: `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` (`crates/nfse-nacional/src/lib.rs`).
- O ambiente é escolhido por empresa. Cancelamento e reconciliação usam o ambiente gravado na DPS, não o atual da empresa.
- Na Produção Restrita o contribuinte precisa estar no cadastro nacional desse ambiente; sem isso a Sefin rejeita com E0084. Por isso os testes reais deste projeto foram feitos em Produção.

## Comunicação

- **mTLS** com o e-CNPJ A1 do prestador (EKU "Client Authentication"), via `reqwest` com `native-tls`, TLS 1.2 no mínimo e sem seguir redirecionamentos (um 3xx no envio vira resposta incerta). A chave de acesso devolvida precisa ter 50 posições (dígitos ou letras maiúsculas) e o XML descompactado no máximo 10 MiB; fora disso, é resposta em formato inesperado.
- **Transporte**: corpo JSON com o XML em GZip + Base64 (`dpsXmlGZipB64`, `pedidoRegistroEventoXmlGZipB64`); função `gzip_base64`.
- **Operações**:

| Operação | Endpoint | Situação |
| --- | --- | --- |
| Emitir | `POST /nfse` | feito |
| Consultar NFS-e | `GET /nfse/{chaveAcesso}` | feito (reconciliação) |
| Consultar DPS | `GET /dps/{id}` | feito (reconciliação e "Consultar na Sefin") |
| Registrar evento | `POST /nfse/{chave}/eventos` | feito para cancelamento (e101101) |
| Consultar evento | `GET /nfse/{chave}/eventos/{tipo}/{nSeq}` | feito para cancelamento (101101, nº 1): "Conferir na Sefin" e "Conferir cancelamentos" em Notas emitidas, e no cancelamento recusado ou sem resposta. Rota confirmada em Produção Restrita (SefinNacional 1.6): responde `{"eventos": [{"chaveAcesso", "tipoEvento", "numeroPedidoRegistroEvento", ...}]}`; o XML vem em `arquivoXml`, codificado em base64 do gzip+base64 (diferente das outras respostas, que usam gzip+base64 simples). Diagnóstico: `nfse-poc consultar-eventos` |
| Distribuição DF-e, parâmetros municipais | — | não implementado |

## Assinatura XMLDSIG

Implementação própria em `crates/nfse-xml/src/assinatura.rs` e `c14n.rs`, conferida contra `xmlsec1` nos testes.

- **DPS e eventos que enviamos**: C14N inclusiva, RSA-SHA1, só o certificado final no `KeyInfo`, **sem prefixo `ds:`** (prefixo gera E1228). Confirmado em Produção.
- **NFS-e devolvida pela Sefin**: C14N exclusiva, RSA-SHA256. O sistema confere a assinatura antes de arquivar, mas só registra aviso se não conferir ([ADR 0007](decisoes/0007-assinatura-da-sefin-como-aviso.md)). A cadeia ICP-Brasil não é validada. Confirmado em Produção Restrita: as duas assinaturas de uma NFS-e real (emitente na DPS e Sefin no `infNFSe`, `exc-c14n#WithComments`) conferem.
- **Evento devolvido pela consulta** (`GET /nfse/{chave}/eventos/…`): a assinatura da Sefin sobre o `infEvento` **não confere**, nem no nosso verificador nem no `xmllint --exc-c14n` (que gera a mesma canonicalização que a nossa). O `pedRegEvento` vem sem a assinatura do autor do pedido; o mais provável é que a Sefin tenha assinado com ela dentro e a removido ao devolver. Por isso o evento não tem a assinatura conferida: vale a conexão mTLS e a conferência de estrutura e chave (`e_cancelamento_da_nfse`).

## Leiaute e XSD

- A ordem dos elementos segue o `xs:sequence` do XSD. A DPS usa o leiaute 1.01, gravado em cada DPS.
- **Leiaute da DPS por ambiente:** a Sefin aplica as Notas Técnicas em datas diferentes na Produção Restrita e na Produção, então Configurações → Leiaute da DPS escolhe, por ambiente, entre `NT 009` e `NT 004` (`LeiauteDps` em `nfse-core`; chaves `leiaute_dps_restrita` e `leiaute_dps_producao` em `configuracao`). Padrão: NT 009 na Produção Restrita e NT 004 na Produção. A escolha é lida a cada emissão e não muda DPS já gravadas. As duas versões gravam `versao="1.01"`.
- **NT 009 × NT 004** (E1235 da Sefin quando o leiaute não é o do ambiente): `finNFSe` sai do grupo `IBSCBS` e fica em `infDPS`, entre `tpEmit` e `cLocEmi`; `cNBS` passa a vir antes de `xDescServ` em `cServ`; `CST` e `cClassTrib` do IBS/CBS sobem para `IBSCBS/valores/trib` (na NT 004 ficam em `gIBSCBS`). Os testes com `xmllint` validam cada leiaute no seu pacote (`v1.01-prodrest-20261020` e `v1.01-prodrest-20260727`).
- Validamos com o XSD de **Produção Restrita** (`xsd/v1.01-prodrest-20261020`): o de Produção tem o padrão da série escrito com `^...$`, que em XSD são literais, e recusa qualquer série ([xsd/README.md](../xsd/README.md), [ADR 0005](decisoes/0005-xsd-de-producao-restrita.md)).
- A validação roda em testes marcados `#[ignore]` que exigem `xmllint` e `xmlsec1` (rodam no CI).

## Id da DPS e numeração

- Id: `DPS` + município IBGE + tipo de inscrição + CNPJ + série + número, 45 caracteres. É a chave de idempotência e é **único por ambiente** (a numeração de cada ambiente começa do 1).
- Série própria (padrão `1`). Pelo Anexo I, 00001–49999 é a faixa de aplicativo próprio; 70000–79999 é do Emissor Web nacional.
- O Anexo I não exige `nDPS` sem buracos nem prevê inutilização. A única regra do número é **E0014**: série e número já usados por uma DPS que gerou NFS-e não podem ser reusados. Como a série `1` pode ter sido usada por outro emissor, o botão "Consultar na Sefin" (`ajustar_numeracao_pela_sefin`) acha o primeiro número livre por busca exponencial e binária. Empresa importada de outra máquina só reserva número depois dessa consulta ([ADR 0015](decisoes/0015-exportacao-de-empresas.md)).
- Nota rejeitada é refeita com o **próximo** número ("Corrigir e emitir"). Ver [ADR 0003](decisoes/0003-serie-propria-e-numeracao.md).

## Regras de preenchimento

- **Competência** (`dCompet`) = data da emissão; o mês de referência vai só na descrição ([ADR 0004](decisoes/0004-competencia-igual-a-emissao.md)).
- **Total de tributos** (Anexo I): ME/EPP informa o % do Simples (`pTotTribSN`, E0712); não optante informa percentuais ou valores federal/estadual/municipal (E0713); MEI não usa `pTotTribSN` (E0710). Validado em `nfse-core` antes de gastar número.
- **Inscrição municipal** na DPS é configurável por ambiente (E0120 quando o cadastro nacional não a tem; E0116 quando o município a exige). Ver [ADR 0011](decisoes/0011-inscricao-municipal-por-ambiente.md).
- **Retenções do serviço** são percentuais (IRRF, CSLL, INSS, PIS, COFINS) e os valores são calculados sobre o valor de cada nota. PIS/COFINS retidos são somados em `vRetCSLL` (NT 007/2026). O serviço define quais retenções valem (sempre, ou só para tomador que retém); o tomador define se retém, por empresa. Ver [ADR 0012](decisoes/0012-retencoes-em-percentual.md). O **tipo de tomador** (privado, órgão da União, estadual, do Distrito Federal ou município, também por empresa) ajusta isso: estado, Distrito Federal e município retêm o IRRF e o INSS, mas não CSLL, PIS nem COFINS (a IN RFB 1.234/2012 obriga só os órgãos da União a reter as contribuições; não há convênio com a RFB nos termos do art. 33 da Lei 10.833/2003), então essas alíquotas do serviço são ignoradas na nota, mesmo em serviço "em toda nota"; privado e federal usam as do serviço. Para tomador público a DPS leva ainda, em `IBSCBS`, `tpOper` (sempre 1, fornecimento com pagamento posterior) e `tpEnteGov` (1 União, 2 Estado, 3 DF, 4 Município), como o Portal Nacional faz; `indFinal` vai sempre (0, ou 1 se marcado no serviço). Tomador pessoa física não retém tributos federais: a DPS com essa retenção é rejeitada na validação (`Dps::validar`), antes de reservar número. Nota sem tomador não tem retenção: `Dps::validar` recusa retenção federal ou ISS retido pelo tomador quando a DPS não tem `toma`, e a revisão não mostra as caixas "Reter nesta nota". Retenção de **até R$ 10,00** é dispensada e não vai para a DPS: o IRRF (Lei 9.430/1996, art. 67; com 1,5%, nota de até R$ 666,66) e PIS, COFINS e CSLL somados (Lei 10.833/2003, art. 31, § 3º; com 4,65%, nota de até R$ 215,05). A nota sai como se o serviço não retivesse aquele grupo (`tpRetPisCofins` 0), mantendo PIS e COFINS de apuração própria, e a revisão avisa o que foi dispensado. Como o limite vale para o total pago no período, a revisão oferece, para o IRRF e para PIS/COFINS/CSLL em separado, reter mesmo assim nesta nota (`ReterNota::irrf_ate_o_limite` e `contribuicoes_ate_o_limite`); ver `federais_com_escolha` e `retencoes_dispensaveis`, em `nfse-emissor/src/montagem.rs`. A revisão só **avisa** (não bloqueia) de retenção federal para optante do Simples; o sistema não valida se a retenção é cabível para o item da LC 116.
- **Texto dos campos `TSString`** (endereço do tomador e da obra, e-mail, informações complementares, motivo da substituição e justificativa do cancelamento): o XSD só aceita caracteres de U+0020 a U+00FF, sem quebra de linha e sem espaço nas pontas. Travessão, aspas curvas ou emoji colados de outro programa são recusados por `Dps::validar`, antes de reservar número (`conferir_tsstring` em `nfse-core`). A descrição do serviço (`TSDesc2000`) aceita quebra de linha. O endereço do tomador é gravado sem espaços nas pontas.
- **PIS/COFINS de apuração própria**: com CST `01` ou `02`, sem PIS/COFINS retidos e com as alíquotas "PIS próprio %" e "COFINS próprio %" no serviço, a DPS leva `vBCPisCofins` (valor da nota), `pAliqPis`, `pAliqCofins`, `vPis` e `vCofins`, com os valores **truncados** no centavo (650,00 × 0,65% = 4,225 vira 4,22), como o Emissor Web. A Sefin desconta `vPis` e `vCofins` da base do IBS/CBS; sem eles a base fica só menos o ISS. Conferido em 19/10/2026 contra um XML do Emissor Web. Com PIS ou COFINS retidos (percentual de retenção no serviço), os mesmos campos vão também, como no Portal Nacional: `vPis`/`vCofins` com os valores retidos (arredondados, iguais aos somados em `vRetCSLL`) e alíquotas da retenção; sem eles o DANFSe mostra "PIS/COFINS - Débito Apuração Própria" zerados. O tributo não retido segue a alíquota própria, truncado.
- **Obra**: grupo de obra na DPS (regra E0370). **IBS/CBS**: grupo `IBSCBS` no cadastro de serviço (CST, cClassTrib, cIndOp); os três vêm de tabelas oficiais embutidas e o emissor não os calcula (ver [desenvolvimento.md](desenvolvimento.md#classificação-tributária-e-indicador-da-operação-do-ibscbs-cclasstrib-e-cindop)); quem calcula alíquotas, reduções e valores de IBS/CBS é a Sefin.
- **Cancelamento**: motivos `erro-na-emissao`, `servico-nao-prestado`, `outros`; justificativa de 15 a 255 caracteres, sem quebra de linha. Sem resposta da Sefin, ou com o pedido recusado, o emissor consulta o evento: se a nota já está cancelada na Sefin (inclusive pelo portal nacional), registra o cancelamento; senão, a nota continua `autorizada` e basta repetir (o Id do pedido é fixo, não duplica). O evento consultado só é aceito se for o `e101101` homologado com a chave da nota (`e_cancelamento_da_nfse`). Cancelamento por substituição (105102) e por análise fiscal (105104) ainda não são consultados. Prazos variam por município. O PDF é regerado com marca d'água.
- **Substituição**: só de nota `autorizada`, da mesma empresa e do mesmo ambiente. A nova DPS é uma emissão comum (próximo número da série, mesmas garantias) com o grupo `subst`: `chSubstda` (chave da nota substituída), `cMotivo` (01 a 05 ou 99) e `xMotivo` opcional (`TSString`, 15 a 255 caracteres). A Sefin cancela a original ao autorizar a substituta (evento 105102, não consultado nem gravado aqui); o banco marca a original como `substituida` na mesma transação que grava a substituta (`registrar_autorizacao`), inclusive quando a autorização chega pela reconciliação, e o PDF da original é regerado com a marca SUBSTITUÍDA. Enquanto uma substituta estiver reservada, enviando ou incerta, outra substituta da mesma nota é recusada (`substituta_em_andamento`). Prazos e regras de quais campos podem mudar são do município e da Sefin; o sistema não os confere.
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
