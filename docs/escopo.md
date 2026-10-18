# Escopo e roadmap

O que o Emissor de NFS-e cobre, o que ficou fora e o que falta fazer. Para o dono do produto e para quem decide o que entra a seguir. Como o sistema funciona por dentro está em [arquitetura.md](arquitetura.md); as regras da Sefin, em [fiscal.md](fiscal.md); as decisões, em [decisoes/](decisoes/).

Legenda: **Feito** · **Pendente** · **Fora do escopo**.

## Visão geral

Emissor desktop de NFS-e, operado por um escritório de contabilidade em nome das empresas clientes, cada uma com seu certificado digital, seus tomadores e seu catálogo de serviços. A integração é a **NFS-e Padrão Nacional (API Sefin Nacional / ADN)**; emissores municipais próprios estão fora do escopo.

**Atores**
- **Operador do escritório**: emite, cancela, consulta e reenvia notas de várias empresas. Cada máquina é de um operador, com banco local próprio.
- **Empresa cliente (prestador)**: dona do CNPJ e do certificado; não opera o sistema.
- **Tomador**: quem recebe a nota (PJ por CNPJ, PF por CPF).

**Objetivos**
- Emitir uma nota recorrente em segundos, com tomador e serviço já cadastrados; o lote do mês emite todas as mensalidades de uma vez.
- Nenhuma nota duplicada nem perdida em queda de conexão (reconciliação por Id da DPS; ver [arquitetura.md](arquitetura.md#fluxo-de-emissão)).
- Certificados dos clientes protegidos em repouso.

## Situação

- **Fase 0 (prova de conceito)** e **Fase 1 (uso diário)**: concluídas em 16/10/2026. O aplicativo emite, cancela e envia por e-mail NFS-e reais em Produção.
- **Fase 2 (instalação e operação)**: em andamento. Instalador MSI, CI e atualização automática prontos.
- Versão atual: ver `crates/app/Cargo.toml`; histórico em [CHANGELOG.md](../CHANGELOG.md).

## Cenário regulatório

Desde 1º/01/2026 todo município precisa operar no padrão nacional, mas pode emitir pelo Emissor Nacional ou manter emissor próprio e só compartilhar as notas com o ADN (LC 214/2025, art. 62). Este sistema atende o primeiro caso. MEI emite sempre pelo Emissor Nacional.

- **Reforma Tributária (IBS/CBS)**: o leiaute ganhou o grupo `IBSCBS` e totalizadores, com sucessivas Notas Técnicas (NT 004, 005, 007, 009). ISS, IBS e CBS convivem até 2032, então **versionar leiaute e XSDs é requisito**. O cadastro de serviço já tem o grupo IBS/CBS (informativo em 2026).
- **Modelo ABRASF** não recebe mais atualizações; integrações municipais seriam legado.
- **Prazos de cancelamento e substituição** são parametrizados por município.

## Requisitos por módulo

**Empresas**
- **Feito** — cadastro, edição e exclusão ([ADR 0010](decisoes/0010-exclusao-de-empresa.md)); consulta pública de CNPJ preenche razão social, município (IBGE), telefone, e-mail e opção Simples/MEI.
- **Feito** — inscrição municipal por ambiente, município emissor, regime (Simples ME/EPP, MEI, não optante), ambiente e série; próximo número por ambiente e série, com ajuste manual e "Consultar na Sefin".
- **Fora do escopo** — logotipo da empresa no DANFSe.
- **Pendente** — incentivos fiscais.

**Certificado A1**
- **Feito** — importar `.pfx`/`.p12` (inclusive BER), extrair CNPJ (OID 2.16.76.1.3.3), titular e validade; bloquear vínculo se o CNPJ do certificado não for da empresa; conferir EKU Client Authentication; guardar cifrado no banco; histórico de certificados; alerta de vencimento ao abrir e lista em Configurações.
- **Pendente** — validar a cadeia ICP-Brasil (exige embarcar as ACs raiz).
- **Fora do escopo** — A3/PKCS#11, DPAPI e CNG ([ADR 0009](decisoes/0009-certificado-a1-e-senha-mestra.md)).
- **Em aberto** — procuração eletrônica (usar o e-CNPJ do escritório): não há indicação de que a API Sefin aceite.

**Tomadores**
- **Feito** — cadastro global reutilizável (`pessoa`) com vínculo por empresa (e-mails, observação, retém impostos federais, retém ISS); busca que sugere tomadores de outras empresas e vincula com um clique; PJ com consulta de CNPJ (BrasilAPI), PF com CPF validado; nota sem tomador identificado; busca de CEP no endereço do tomador e da obra.
- **Pendente** — cache e alternativa à consulta de CNPJ; nome do município pela tabela IBGE (hoje só vem da consulta de CNPJ).
- **Pendente** — tomador estrangeiro (NIF, país) e intermediário do serviço.

**Catálogo de serviços**
- **Feito** — descrição padrão com variáveis, `cTribNac`, NBS, código municipal, ISSQN, PIS/COFINS, retenções em percentual, total aproximado de tributos, valor sugerido e grupo IBS/CBS; importação e exportação por planilha (Excel, ODS ou CSV).
- **Pendente** — tabela nacional de códigos (Anexo B) e validação contra a parametrização do município.

**Emissão**
- **Feito** — fluxo empresa → tomador → serviço → valor → mês de referência → **Revisar** (DPS completa, destaque de Produção, erros) → emitir.
- **Feito** — notas recorrentes e lote do mês, importação de planilha (XLSX, XLS, ODS, CSV) para convênios, "Corrigir e emitir" (rejeitada), "Copiar" (autorizada ou cancelada), envio por e-mail (PDF + XML) pelo SMTP do escritório, arquivamento em `<pasta>/<EMPRESA - CNPJ>/<AAAA-MM>/`.
- **Fora do escopo** — responder automaticamente a e-mails de convênios (fica manual).
- **Pendente** — agendamento automático do lote.

**Pós-emissão**
- **Feito** — cancelamento com motivo e justificativa; reconciliação das incertas.
- **Pendente** — substituição de nota; consulta dos demais eventos (o cancelamento já é conferido sob demanda) e sincronização periódica; download pelo ADN de notas emitidas fora do sistema; manifestação do tomador.

**Relatórios e integração contábil**
- **Feito** — lista de notas por empresa e mês, com situação, e-mail enviado e abertura do PDF/pasta. O XML autorizado é gravado puro na pasta, para importação por sistema contábil.
- **Pendente** — relatórios (faturamento, ISS devido e retido, IBS/CBS), pacote ZIP mensal de XMLs, painel do dia.

**Auditoria e acesso**
- **Feito** — senha mestra por máquina e log de auditoria (empresa, certificado, nota autorizada, e-mail, avisos); botão Bloquear.
- **Fora do escopo** — usuários, perfis e restrição por empresa.
- **Pendente** — bloqueio automático por inatividade.

## Segurança, LGPD e guarda

O banco concentra os certificados de todos os clientes; quem copia o arquivo e descobre a senha mestra pode emitir notas em nome de qualquer empresa.

- **Feito** — certificados e senha do SMTP cifrados em repouso; chave derivada da senha mestra; comandos IPC exigem o cofre aberto; `abrir_arquivo` só abre pdf/xml/csv dentro da pasta de saída.
- **Feito** — repositório público sem segredos nem dados de clientes (`.gitignore` bloqueia `.pfx`, `.db`, XMLs e `certs/`).
- **Feito** — integridade: hash do XML autorizado conferido a cada leitura.
- **Feito** — consulta de CNPJ e CEP: só o documento ou o CEP sai da máquina.
- **Feito** — banco inteiro cifrado em repouso com SQLCipher (chave derivada da senha mestra; [ADR 0013](decisoes/0013-banco-cifrado-sqlcipher.md)): nomes, CPFs, e-mails e valores não ficam legíveis no `emissor.db`.
- **Pendente (LGPD)** — exportar ou anonimizar tomador PF sem notas. Os XML e PDF da pasta de saída continuam em claro (usar criptografia de disco).
- **Feito** — backup cifrado do banco (que contém os XMLs autorizados), automático ao abrir o sistema e manual, com teste de restauração. 
- **Pendente** — exportação ZIP da guarda de XML (confirmar o prazo de guarda com o contador responsável).
- **Logs**: nunca registrar senha, chave privada ou XML completo.

## Riscos

| Risco | Situação |
| --- | --- |
| Layout muda a cada Nota Técnica (IBS/CBS até 2032) | XSDs versionados e testes; acompanhar o portal NFS-e |
| Timeout gera nota duplicada | Mitigado por reconciliação pelo Id da DPS |
| Série já usada por outro emissor (E0014) | Mitigado por "Consultar na Sefin" |
| Produção Restrita sem o contribuinte cadastrado (E0084) | Testes reais em Produção |
| Vazamento do banco com certificados | Cofre com senha mestra; backup cifrado |
| Limites das APIs públicas de CNPJ | Cache e alternativa pendentes |
| Parametrização municipal divergente (E0312, E0116) | Orientação por código; consulta de parâmetros pendente |
| Antispam do provedor de e-mail | Cabeçalhos completos; falha registrada e reenvio pelo botão "E-mail" |
| Sem CI para macOS; testes no Linux incompletos | Ver [desenvolvimento.md](desenvolvimento.md#problemas-conhecidos) |

## Roadmap

**Fase 0 — prova de conceito** · concluída: certificado A1, mTLS, XMLDSIG e leiaute aceitos pela Sefin; primeira NFS-e real autorizada e cancelada.

**Fase 1 — uso diário** · concluída: banco e cofre, emissão confiável, conferência da assinatura da Sefin, DANFSe, aplicativo desktop, recorrentes, importação de planilha, e-mail.

**Fase 2 — instalação e operação** · em andamento
- **Feito** — instalador MSI, CI que o gera, atualização automática, logomarca oficial no DANFSe.
- **Pendente** — assinatura de código (evita o alerta do SmartScreen); log de erros exportável; tabela de municípios IBGE; validação da cadeia ICP-Brasil; cache da consulta de CNPJ; testes de integração dos comandos do app.

**Fase 3 — pós-emissão e integração** · pendente: substituição; consulta dos demais eventos; distribuição DF-e pelo ADN; manifestação do tomador; parâmetros municipais; relatórios e ZIP mensal; tomador estrangeiro e intermediário.

## Fora do escopo

Certificado A3; emissores municipais próprios; PostgreSQL e uso em rede; login, perfis e permissões por empresa; resposta automática a e-mails de convênios; Lucro Real (atende Simples Nacional e Lucro Presumido).

## Fontes

- [Portal NFS-e — documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica)
- [NT 007 SE/CGNFS-e](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf)
- [LC 214/2025 e NFS-e nacional — Senior](https://documentacao.senior.com.br/exigenciaslegais/noticias/federal/2025/2025-02-20-nota-fiscal-de-servicos-eletronica-de-padrao-nacional-para-2026/)
- [Fim das atualizações ABRASF — Convergência Digital](https://convergenciadigital.com.br/mercado/associacao-das-secretarias-de-financas-decide-pelo-fim-das-atualizacao-da-nfs-e/)
