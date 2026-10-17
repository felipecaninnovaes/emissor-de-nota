# Escopo — Emissor de NFS-e Multiempresa (Rust)

Criado em 16/10/2026 · atualizado em 16/10/2026 com o estado real do projeto · @Felipe Canin Novaes

## Estado em 16/10/2026

- **Fase 0 concluída.** Assinatura XMLDSIG, mTLS com o e-CNPJ A1 e leiaute da DPS aceitos pela Sefin. Primeira NFS-e real autorizada em **Produção** (e cancelada). A Produção Restrita não serve de teste para o escritório: o contribuinte não está no cadastro nacional desse ambiente (rejeição E0084).
- **Fase 1 concluída no código.** Aplicativo desktop (Tauri 2) emitindo e cancelando notas reais pelo próprio sistema, com recorrentes, lote do mês, importação de planilha e envio por e-mail. Recorrentes, planilha e e-mail ainda estão em validação no uso do dia a dia.
- **Próximo:** Fase 2 (instalador e operação), depois Fase 3 (pós-emissão e integração). Ver [Roadmap](#roadmap).
- Detalhe do andamento, decisões técnicas e pendências: [`docs/ESTADO.md`](docs/ESTADO.md).

Legenda usada nos módulos: **Feito** · **Pendente** (Fase 2 ou 3) · **Fora do escopo**.

## Visão geral e objetivos

O produto é um emissor desktop de NFS-e, operado pelo escritório de contabilidade em nome das empresas clientes, cada uma com seu próprio certificado digital, cadastro de tomadores e catálogo de serviços. A integração é a **NFS-e Padrão Nacional (API Sefin Nacional / ADN)**; emissores municipais próprios ficaram fora do escopo (todos os municípios dos clientes estarão no ambiente nacional até nov/2026).

**Atores**

- **Operador do escritório**: emite, cancela, consulta e reenvia notas para várias empresas no mesmo dia. Cada máquina é de um operador, com um banco local próprio.
- **Empresa cliente (prestador)**: dona do CNPJ e do certificado; recebe a nota e os arquivos, mas não opera o sistema.
- **Tomador**: quem recebe a nota (PJ via CNPJ, PF via CPF). Estrangeiro ainda não modelado.

**Objetivos**

- Emitir uma nota recorrente em menos de 30 segundos com tomador e serviço já cadastrados — hoje o lote do mês emite todas as mensalidades de uma vez.
- Volume: 35 empresas, média de 500 notas/mês (cerca de 25 por dia útil); SQLite local atende com folga.
- Zero nota duplicada e zero nota perdida em queda de conexão — garantido pela reconciliação por Id da DPS e pelo vínculo gravado antes do envio (recorrência e planilha).

## Cenário regulatório (out/2026)

Desde 1º/01/2026 todo município precisa operar no padrão nacional, mas **pode emitir pelo Emissor Nacional ou manter emissor próprio e só compartilhar as notas com o ADN** ([LC 214/2025, art. 62 — Senior](https://documentacao.senior.com.br/exigenciaslegais/noticias/federal/2025/2025-02-20-nota-fiscal-de-servicos-eletronica-de-padrao-nacional-para-2026/); [NotaGateway](https://notagateway.com.br/blog/tudo-sobre-nfs-e-nacional/)).

| Situação do município | Onde o software emite | Exemplos |
| --- | --- | --- |
| Usa o Emissor Nacional | API Sefin Nacional (padrão único) | Catanduva/SP desde 01/09/2026 ([NotaGateway](https://notagateway.com.br/blog/catanduva-sp-muda-emissao-da-nfs-e-para-o-emissor-nacional/)) |
| Emissor próprio, layout nacional | Webservice da prefeitura (layout próximo ao nacional) | Teresópolis/RJ ([comunicado](https://www.teresopolis.rj.gov.br/wp-content/uploads/2025/12/Comunicado-Nota-Padrao-Nacional.pdf)) |
| Emissor próprio, layout municipal | Webservice próprio da prefeitura | São Paulo capital e DF ([Contábeis](https://www.contabeis.com.br/forum/tributos-estaduais-municipais/412710/nota-fiscal-eletronica-municipal-e-nfse/); [Actana](https://actana.com.br/blog/quais-municipios-ja-aderiram-a-nfs-e-nacional-veja-o-mapa-atualizado)) |
| MEI | Sempre Emissor Nacional (obrigatório desde 2023) | Qualquer município |

**Pontos que mudam o escopo**

- **Catanduva** (sede do escritório) migrou para o Emissor Nacional em 01/09/2026; guias de ISSQN e obrigações acessórias continuam no sistema municipal ([NotaGateway](https://notagateway.com.br/blog/catanduva-sp-muda-emissao-da-nfs-e-para-o-emissor-nacional/)). Particularidade confirmada na prática: o escritório é Simples Nacional com **ISS fixo** no município, e a prefeitura exige a inscrição municipal na DPS em Produção (E0116).
- **Modelo ABRASF** não recebe mais atualizações ([Convergência Digital](https://convergenciadigital.com.br/mercado/associacao-das-secretarias-de-financas-decide-pelo-fim-das-atualizacao-da-nfs-e/)); integrações municipais seriam legado e ficaram fora do escopo.
- **Reforma Tributária (IBS/CBS)**: o layout ganhou o grupo `IBSCBS` na DPS e totalizadores na NFS-e, com sucessivas Notas Técnicas (NT 004, 005, 007, 009) e o campo `cIndOp` ([NT 007 — gov.br](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf); [Jornal Contábil — NT 009](https://jornalcontabil.com.br/noticia/nfs-e-publicadas-atualizacoes-sobre-o-layout-do-padrao-nacional-que-inclui-regras-para-ibs-e-cbs/)). ISS, IBS e CBS convivem até 2032: **versionar o leiaute e os XSDs é requisito**. O modelo já tem o grupo IBS/CBS (informativo em 2026).
- **Prazos de cancelamento e substituição** são parametrizados por município no Sistema Nacional; não há prazo único ([Buscador NCM](https://buscadorncm.com.br/nfse/eventos)).
- **Código de serviço**: `cTribNac` de 6 dígitos sem pontos, derivado da LC 116; NBS e código municipal conforme parametrização. Um código não habilitado para a empresa no município gera E0312.

## Integração técnica — API Sefin Nacional / ADN

O software envia uma **DPS (Declaração de Prestação de Serviço)** assinada; a Sefin valida, gera a NFS-e em XML com a DPS encapsulada e devolve uma chave de acesso de 50 dígitos ([NotaGateway — NT 004](https://notagateway.com.br/?p=5546); [click-nfse](https://packagist.org/packages/noclick11/click-nfse)).

**Requisitos de comunicação (confirmados em Produção)**

- **mTLS** com o e-CNPJ A1 do prestador, com EKU "Client Authentication" ([NotaGateway](https://notagateway.com.br/?p=6164)).
- **Assinatura XMLDSIG** da DPS e dos eventos: C14N inclusiva, RSA-SHA1, só o certificado final no KeyInfo, sem prefixo `ds:` (prefixo gera E1228). A NFS-e devolvida vem assinada pela Sefin com C14N exclusiva e RSA-SHA256; o sistema confere essa assinatura antes de arquivar.
- **Transporte**: corpo JSON com o XML compactado em GZip e codificado em Base64 (`dpsXmlGZipB64`, `pedidoRegistroEventoXmlGZipB64`).
- **Ordem dos elementos**: segue o `xs:sequence` do XSD. A validação contra o XSD roda nos testes (CI). O XSD de Produção de 09/02/2026 tem o padrão da série escrito com `^...$` e recusa qualquer série; usamos o pacote de Produção Restrita (v1.01-20260727).
- **Ambientes**: Produção Restrita e Produção, escolhidos por empresa. A Produção Restrita não tem o escritório cadastrado (E0084), então os testes reais são feitos em Produção com notas que seriam emitidas de qualquer forma.

**Operações**

| Operação | Endpoint | Situação |
| --- | --- | --- |
| Emitir | `POST /nfse` | **Feito** |
| Consultar NFS-e | `GET /nfse/{chaveAcesso}` | **Feito** (reconciliação) |
| Consultar DPS | `GET /dps/{id}` | **Feito** — reconciliação após timeout e "Consultar na Sefin" (descobre números já usados) |
| Registrar evento | `POST /nfse/{chave}/eventos` | **Feito** para cancelamento (e101101), testado em Produção · **Pendente**: substituição e manifestação |
| Consultar eventos | `GET /nfse/{chave}/eventos` | **Pendente** (Fase 3) |
| Distribuição DF-e | ADN, paginado por NSU | **Pendente** (Fase 3) |
| Parâmetros municipais | API de parametrização | **Pendente** (Fase 3) |

**Regras críticas de negócio (como ficaram)**

- **Idempotência**: o Id da DPS (`DPS` + município IBGE + tipo + CNPJ + série + número, 45 caracteres) é a chave. Antes do envio a DPS é gravada como "enviando"; em timeout vira "incerta" e o sistema consulta `GET /dps/{id}` antes de qualquer reenvio. O Id é único **por ambiente** (a numeração de cada ambiente começa do 1).
- **Numeração** (Anexo I, v1.01-20260209): série 00001–49999 é a faixa de aplicativo próprio (70000–79999 é o Emissor Web); o número não precisa ser sequencial sem buracos e não existe inutilização de DPS. A única regra é a **E0014**: série e número que já geraram NFS-e não podem ser reusados — por isso a série 1 pode ter sido usada por outro emissor, e o botão "Consultar na Sefin" acha o primeiro número livre (busca exponencial + binária). Nota rejeitada é refeita com o próximo número ("Corrigir e emitir").
- **Competência** (`dCompet`) = data da emissão; o mês de referência do serviço vai só na descrição (`{mes_ref}`).
- **Total de tributos** (Anexo I): ME/EPP informa o % do Simples (`pTotTribSN`, E0712); não optante informa percentuais ou valores federal/estadual/municipal (E0713); MEI não usa `pTotTribSN` (E0710). Validado antes de gastar número.
- **DANFSe**: PDF gerado localmente a partir do XML autorizado (layout v2.0 da NT 008/2026), com QR Code da consulta pública e marca d'água de cancelada.
- **Erros**: cada rejeição mostra o texto da Sefin e uma orientação em português (E0014, E0084, E0116, E0120, E0312, E0710/E0712/E0713, E0714/E0717, E1228, E1235, E2400, E2404).
- **Contingência**: não há contingência offline; com a Sefin fora a nota fica "incerta" e é reconciliada ao abrir o sistema ou pelo botão "Verificar".
- **Rate limit (429)**: o lote espera e tenta mais uma vez; a tentativa recusada por limite não gera nota.

## Certificado digital por empresa

Cada empresa tem um e-CNPJ A1 (.pfx), que assina a DPS/eventos e autentica a conexão mTLS. **A3 ficou fora do escopo** (90% dos clientes já têm A1; os demais vão providenciar).

- **Feito** — Importar .pfx/.p12 com senha (inclusive arquivos em BER com comprimento indefinido, como alguns gerados pelas ACs e pelo Windows); extrair CNPJ (OID 2.16.76.1.3.3), titular e validade; **bloquear vínculo se o CNPJ do certificado não for da empresa** (pela raiz do CNPJ).
- **Feito** — Guardar o .pfx e a senha **cifrados no banco** (AES-256-GCM, chave derivada da senha mestra com Argon2id, contexto por registro); decifrar só em memória.
- **Feito** — Conferir EKU Client Authentication.
- **Feito** — Vencimentos: alerta ao abrir (30 dias) e lista em Configurações (60 dias); validade no topo da tela.
- **Feito** — Histórico de certificados (trocar não perde notas antigas).
- **Pendente** — Validar a cadeia ICP-Brasil com bundle de ACs embarcado.
- **Fora do escopo** — DPAPI do Windows e repositório CNG (decidido: só senha mestra); A3/PKCS#11.
- **Em aberto** — Procuração eletrônica (escritório usar o próprio e-CNPJ): não há indicação de que a API Sefin aceite.

**Risco conhecido**: relatos de E0717 com alguns certificados "RFB e-CNPJ A1" em eventos de manifestação ([fórum ACBr](https://www.projetoacbr.com.br/forum/topic/91333-erro-e0717-ao-registrar-evento-de-manifesta%C3%A7%C3%A3o-de-tomador-%E2%80%94-certificado-icp-brasil-e-cnpj-rfb-a1-rejeitado-pelo-webservice/)). A CLI `nfse-poc cert-info` faz o diagnóstico (validade, titular, EKU, CNPJ).

## Requisitos funcionais por módulo

**1. Empresas (prestadores)**

- **Feito** — Cadastro, edição e exclusão (sem notas: apaga; com notas: arquiva e o recadastro do CNPJ traz de volta com o histórico). Consulta pública de CNPJ preenche razão social, município (IBGE), telefone, e-mail e opção Simples/MEI.
- **Feito** — Inscrição municipal (e se vai na DPS, por ambiente), município emissor, regime (Simples ME/EPP, MEI, não optante; apuração; regime especial), ambiente, série. Troca de Homologação para Produção pela edição.
- **Feito** — Próximo número por ambiente e série, ajuste manual e "Consultar na Sefin".
- **Pendente** — Logotipo da empresa no DANFSe; incentivos fiscais.

**2. Tomadores (clientes das empresas)**

- **Feito** — **Cadastro global reutilizável** (`pessoa`) com vínculo por empresa (`empresa_tomador`: e-mails de envio, observação). Um tomador cadastrado por uma empresa é vinculado a outra automaticamente na importação de planilha.
- **Feito** — PJ com consulta pública de CNPJ (publica.cnpj.ws, 3 consultas/minuto; avisa situação cadastral diferente de "Ativa"). PF com CPF validado pelo dígito verificador.
- **Feito** — Nota sem tomador identificado.
- **Pendente** — Cache e fallback entre provedores de CNPJ; autocompletar CEP; nome do município pela tabela IBGE (hoje só vem pela consulta de CNPJ).
- **Pendente** — Tomador estrangeiro (NIF, país) e intermediário do serviço.

**3. Catálogo de serviços por empresa**

- **Feito** — Descrição padrão com `{mes_ref}`, `cTribNac`, NBS, código municipal, ISSQN (tributação, retenção, alíquota), PIS/COFINS (CST, tipo de retenção), retenções em percentual (IRRF, CSLL, INSS, PIS, COFINS — valores calculados sobre cada nota), total aproximado de tributos e valor sugerido.
- **Pendente** — Tabela nacional de códigos (Anexo B) e validação contra a parametrização do município; classificação IBS/CBS na tela.

**4. Emissão**

- **Feito** — Fluxo: empresa → tomador (busca por nome/documento) → serviço → valor → mês de referência → **Revisar** (DPS completa, destaque de Produção, erros de validação) → emitir.
- **Feito** — **Notas recorrentes** (mensalidades) e **Lote do mês** de todas as empresas, uma nota por recorrência e mês, com andamento e sem parar o lote numa falha; "Tornar recorrente" depois de emitir.
- **Feito** — **Importação de planilha** (XLSX, XLS, ODS, CSV) para convênios, uma nota por linha, com "Cadastrar tomador" direto da linha; reimportar a mesma planilha não repete notas já autorizadas.
- **Feito** — "Corrigir e emitir" a partir de uma nota rejeitada.
- **Feito** — **Envio por e-mail** (PDF + XML) pelo SMTP do escritório, automático após emitir (opcional) ou pelo botão "E-mail"; cada envio registrado. *Em validação com o servidor do escritório.*
- **Feito** — XML e PDF gravados em `<pasta>/<EMPRESA - CNPJ>/<AAAA-MM>/`.
- **Pendente** — Agendamento automático do lote (hoje o operador dispara).

**5. Pós-emissão (eventos)**

- **Feito** — Cancelamento com motivo e justificativa (PDF regravado com marca d'água).
- **Feito** — Reconciliação das incertas ao abrir o sistema e pelo botão "Verificar".
- **Pendente** — Substituição (nova DPS vinculada) respeitando prazos por município.
- **Pendente** — Consulta de eventos e sincronização periódica; download pelo ADN de notas emitidas fora do sistema; manifestação do tomador.

**6. Relatórios e integração contábil**

- **Feito** — Lista de notas por empresa e mês, com situação, e-mail enviado e abertura do PDF/pasta; o Prosoft importa o XML puro direto da pasta.
- **Pendente** — Relatórios (faturamento, ISS devido e retido, IBS/CBS), pacote ZIP mensal de XMLs, painel do dia.

**7. Auditoria**

- **Feito** — Senha mestra por máquina (sem login/perfis, decisão do escritório) e log de auditoria (empresa cadastrada/editada/excluída, certificado importado, nota autorizada, e-mail enviado, avisos).
- **Fora do escopo** — Usuários, perfis e restrição por empresa.

## Modelo de dados (como está)

O ponto central é separar **pessoa** (dado público, único por documento) de **vínculo** (relação de uma empresa com aquele tomador).

| Tabela | Conteúdo | Observação |
| --- | --- | --- |
| `pessoa` | tipo, documento único, nome, endereço, IBGE, contato, fonte dos dados | Prestadores e tomadores |
| `empresa` | pessoa, IM (e se vai na DPS por ambiente), município emissor, regime, ambiente, série, ativa | Arquivada quando excluída com notas |
| `numeracao` | empresa, ambiente, série, próximo número | Reserva atômica |
| `certificado` | empresa, pfx e senha cifrados, thumbprint, titular, validade, ativo | Histórico |
| `empresa_tomador` | empresa, pessoa, e-mails de envio, observação | Vínculo reutilizável |
| `servico` | empresa, descrição, códigos, tributação (JSON), valor sugerido, ativo | |
| `dps` | empresa, ambiente, série, número, Id (único por ambiente), situação, modelo, XML assinado, tentativas, erro | 0..1 NFS-e |
| `nfse` | DPS, chave, número, XML autorizado e seu SHA-256, valor, competência | Documento fiscal |
| `evento` | NFS-e, tipo, XML | Cancelamento |
| `recorrencia` · `emissao_recorrente` | modelo da mensalidade; DPS de cada mês | Uma nota por mês |
| `emissao_importada` | chave de conteúdo da linha da planilha, DPS | Reimportar não duplica |
| `envio_email` | NFS-e, destinatários, assunto, erro | Registro de cada envio |
| `configuracao` · `auditoria` | pasta, SMTP (senha cifrada), cabeçalho do cofre; ações | |

**Situação da DPS**: reservada → enviando → autorizada / rejeitada / incerta (aguardando reconciliação) → cancelada / substituída.

**Regras**

- Guardar **sempre o XML autorizado original** com hash (detecta alteração no banco); valores em colunas só para listagem.
- Número reservado em transação atômica.
- Versão do leiaute gravada em cada DPS.
- Migrações com `sqlx migrate`, só SQLite. Recriações de tabela que exigem desligar chaves estrangeiras rodam como "ajuste" logo após as migrações (o sqlx 0.8 sempre usa transação).

## Arquitetura e stack Rust

Workspace Cargo com o **núcleo fiscal isolado da interface**; a Sefin fica atrás de um trait (`Sefin`/`Conector`) que os testes simulam.

**Crates**

- `nfse-core`: tipos de domínio (DPS, documentos, valores em centavos) e validações do leiaute/Anexo I, sem I/O.
- `nfse-cert`: certificado A1 (PKCS#12, inclusive BER), dados do titular, assinatura RSA.
- `nfse-xml`: geração da DPS e dos eventos na ordem do XSD, C14N (inclusiva e exclusiva), assinatura e conferência XMLDSIG, leitura da NFS-e.
- `nfse-nacional`: cliente mTLS da Sefin e orientações por código de rejeição.
- `nfse-db`: banco SQLite, cofre (Argon2id + AES-256-GCM), cadastros, numeração, DPS/NFS-e, recorrências, importações, e-mail.
- `nfse-emissor`: emissão confiável, reconciliação, cancelamento, arquivamento em pasta, planilha, e-mail.
- `nfse-pdf`: DANFSe v2.0.
- `nfse-poc`: CLI de diagnóstico (cert-info, gerar/assinar/verificar DPS, emitir, consultar, cancelar).
- `app` (`emissor-nfse`): aplicativo Tauri 2, interface em HTML/CSS/JS sem bundler.

**Bibliotecas escolhidas**

| Necessidade | Escolha | Observação |
| --- | --- | --- |
| Interface desktop | Tauri 2 (WebView2) | HTML/CSS/JS puro, sem Node, para manutenção por uma pessoa |
| Banco | `sqlx` 0.8 (SQLite) | WAL, `synchronous = FULL` |
| HTTP + mTLS | `reqwest` com `native-tls` (Identity PKCS#12) | |
| XML | `roxmltree` (leitura) e gerador próprio na ordem do XSD | |
| Validação XSD | `xmllint` nos testes do CI | Não embarcada no app (a DPS é montada a partir do modelo validado) |
| Assinatura XMLDSIG | Implementação própria (C14N + `rsa` + `sha1`/`sha2`) | Conferida contra `xmlsec1` no CI; risco técnico da Fase 0 resolvido |
| PKCS#12 | `p12-keystore` + conversão BER→DER própria | |
| Criptografia em repouso | `aes-gcm`, `argon2`, `zeroize` | |
| PDF | `krilla` + `qrcode` | Fontes do sistema (Arial / Microsoft Sans Serif) |
| E-mail | `lettre` (SMTP, STARTTLS/SSL) | Message-ID e EHLO com o domínio do remetente |
| Planilhas | `calamine`, `csv`, `encoding_rs` | |
| Consulta de CNPJ | publica.cnpj.ws | |
| Assíncrono | `tokio` | |

`rust-version` = 1.88.

**Banco**: um SQLite por máquina (decisão do escritório: cada máquina é de um operador e algumas empresas não podem ser vistas por outros usuários). PostgreSQL e uso em rede ficaram **fora do escopo**. Não usar o arquivo em pasta de rede compartilhada.

**Distribuição** (**Pendente**, Fase 2): instalador MSI (`cargo tauri build`), CI que gera o instalador, assinatura de código (evita o alerta do SmartScreen) e atualização automática.

## Segurança, LGPD, backup e guarda

O banco concentra os certificados de todos os clientes: quem copia o arquivo e descobre a senha mestra pode emitir notas em nome de qualquer empresa. A segurança dos certificados é o requisito não funcional mais importante.

- **Feito** — Certificados e senha do SMTP cifrados em repouso com chave derivada da senha mestra (Argon2id); decifrados só em memória e zerados depois (`zeroize`).
- **Feito** — Acesso por senha mestra por máquina (sem recuperação).
- **Feito** — Repositório público sem segredos nem dados de clientes: certificados de teste fictícios, `.gitignore` para `.pfx`, bancos, XMLs e a pasta `certs/`.
- **Feito** — Integridade: hash do XML autorizado conferido a cada leitura.
- **Feito** — Consulta de CNPJ: só o CNPJ sai para a API pública; a fonte fica registrada no cadastro.
- **LGPD**: CPF, nome e endereço de PF são dados pessoais; base legal é a obrigação fiscal; não enviar dados a terceiros além dos fiscais. **Pendente** — exportar/anonimizar tomador PF sem notas.
- **Logs**: nunca registrar senha, chave privada ou XML completo em log.
- **Pendente** — Backup automático diário do SQLite com retenção e teste de restauração (destino: pasta de rede ou nuvem do escritório).
- **Guarda de XML**: XML autorizado no banco e na pasta por empresa/mês (mínimo de 5 anos; confirmar a regra do escritório). **Pendente** — exportação em ZIP.

## Riscos, perguntas em aberto e roadmap

**Riscos**

| Risco | Impacto | Situação |
| --- | --- | --- |
| Assinatura XMLDSIG em Rust sem crate madura | Bloqueia a emissão | **Resolvido** na Fase 0: implementação própria aceita pela Sefin em Produção |
| Layout muda a cada Nota Técnica (IBS/CBS até 2032) | Rejeições após atualização | XSDs versionados, testes automatizados; acompanhar o portal NFS-e |
| Timeout gera nota duplicada | Duplicidade fiscal | **Mitigado**: reconciliação por Id da DPS; vínculo de recorrência/planilha gravado antes do envio |
| Série já usada por outro emissor | Rejeição E0014 em sequência | **Mitigado**: "Consultar na Sefin" ajusta a numeração |
| Produção Restrita sem o contribuinte | Não dá para homologar | Testes em Produção com notas reais do escritório |
| Vazamento do banco com certificados | Emissão fraudulenta | Cofre com senha mestra; **pendente** backup cifrado |
| Limites das APIs públicas de CNPJ | Cadastro lento | Aviso de limite; **pendente** cache e fallback |
| Parametrização municipal divergente | Rejeição (E0312, E0116) | Orientação por código; **pendente** consulta de parâmetros municipais |
| Antispam do provedor de e-mail | Nota não chega ao tomador | Cabeçalhos completos; falha registrada e reenvio pelo botão "E-mail" |
| Disco cheio pelo cache de compilação (`target/`) | Build falha na máquina de desenvolvimento | `cargo clean` periódico |

**Perguntas para o escritório** (respondidas)

- [x] Quantas empresas, quantas notas por mês e quantos operadores simultâneos? 35
- [x] Em quais municípios estão os clientes, e quais usam o Emissor Nacional? Principalmente Catanduva, mas em Novembro/2026 todos os municípios deveram estar homologados no ambiente nacional.
- [x] Algum cliente com certificado A3? Todos já têm e-CNPJ? 90% já tem E-CNPJ, os que não tiverem vai ser solicitado para fazer.
- [x] Quais regimes e retenções são comuns (ISS retido, INSS, IR, PIS/COFINS/CSLL)? ISS RETIDO, IRRF, CSLL, PIS, COFINS.
- [x] Há notas recorrentes (mensalidades) e emissão em lote? Sim, o proprio escritorio tem as mensalidades que não mudam os valores apenas na descrição o mês que refere o serviço, e muitos dos clientes/empresas são clinicas, tem bastante notas de convenio.
- [x] Qual sistema contábil/fiscal recebe os dados e em que formato? Eles usam prosoft que importa os xmls, não tem nenhum sistema de emissão atualmente.
- [x] O cliente final precisa acessar algo (portal, PDF por e-mail)? Boa parte é por e-mail, mas vai ser necessário tem uma pasta separado por empresas em meses xml e pdf pois alguns convênios obriga que seja respondido o e-mail.
- [x] Rede local com servidor ou máquinas isoladas (define SQLite x PostgreSQL)? Até tem servidor mas é mais complexo e certas empresas outros usuarios não podem ter acesso, então o foco será por maquina
- [x] Quem dá suporte e como as atualizações chegam às máquinas? Eu mesmo
- [x] Modelo comercial: uso interno ou produto para vender a outros escritórios (muda licenciamento, multi-tenant e homologação)? Uso interno mas o repositório do projeto vai ser publico.

**Decisões a partir das respostas**

| Tema | Decisão | Impacto no escopo |
| --- | --- | --- |
| Volume | 35 empresas | SQLite atende com folga |
| Municípios | Principalmente Catanduva; todos no ambiente nacional até nov/2026 | Só o adaptador Sefin Nacional; emissores municipais saem do escopo |
| Certificado | 90% com e-CNPJ A1; os demais vão providenciar | Só A1 (.pfx); A3 fora do escopo |
| Retenções | ISS retido, IRRF, CSLL, PIS, COFINS | Cadastro de serviço e emissão precisam desses cinco desde o MVP |
| Recorrentes | Mensalidades com valor fixo; só muda o mês de referência na descrição | Modelo de nota com variável na descrição (ex.: `{mes_ref}`) e geração em lote; sobe para a Fase 1 |
| Perfil dos clientes | Muitas clínicas, com muitas notas de convênio | Convênios como tomadores PJ reutilizáveis entre várias clínicas; emissão em lote por convênio |
| Sistema contábil | Prosoft importa os XMLs; não há emissor atual | Exportar XMLs no formato que o Prosoft importa; sem migração de dados |
| Entrega ao tomador | E-mail na maioria; pasta por empresa e mês com XML e PDF | Pasta configurável `<empresa>/<AAAA-MM>/` com XML e PDF de cada nota |
| Infraestrutura | Foco por máquina; alguns usuários não podem ver certas empresas | Um banco SQLite por máquina; PostgreSQL em rede fica fora do escopo |
| Suporte | Você mesmo | Atualização automática e log de erros exportável para diagnóstico |
| Modelo | Uso interno, repositório público | Nada de segredo, certificado ou dado de cliente no repositório; escolher licença; CI que gera o instalador |

**Respostas complementares**

- [x] Quantas notas por mês no total e quantos operadores (máquinas)? Media 500 notas
- [x] Regimes das empresas (Simples, Presumido, Real)? Define cálculo de ISS, retenções e IBS/CBS. Simples e presumido.
- [x] O que é "responder o e-mail" exigido por alguns convênios: enviar a nota como resposta ao e-mail recebido do convênio (exige ler a caixa de entrada) ou só enviar para um endereço específico? De inicio isso ficara manual para o usuário, não vamos inflar o escopo.
- [x] Qual layout o Prosoft espera na importação (XML da NFS-e nacional puro ou outro formato)? XML Puro
- [x] Usuários diferentes na mesma máquina precisam de login separado, ou cada máquina é de um operador? Cada maquina é um operador.

**Decisões complementares**

| Tema | Decisão | Impacto no escopo |
| --- | --- | --- |
| Volume | Média de 500 notas/mês (cerca de 25 por dia útil) | SQLite local sobra; o gargalo é o tempo de digitação, então recorrentes e lote valem mais que desempenho |
| Regimes | Simples Nacional e Lucro Presumido | Lucro Real fora do escopo. Simples: informar optante e alíquota da faixa, sem destacar PIS/COFINS próprios. Presumido: ISS pela alíquota municipal e retenções federais configuráveis por serviço |
| E-mail dos convênios | Manual pelo usuário por enquanto | Fora do escopo; o sistema só gera os arquivos na pasta e envia o e-mail simples ao tomador |
| Prosoft | Importa o XML puro | Exportação = cópia do XML autorizado, sem conversão |
| Usuários | Cada máquina é de um operador | Sem perfis nem permissões por empresa; fica só uma senha local para abrir o app e proteger os certificados. O módulo 7 se reduz a log de auditoria |

**Decisões tomadas durante o desenvolvimento**

| Tema | Decisão |
| --- | --- |
| Interface | Tauri 2 com HTML/CSS/JS puro (sem Node/bundler) |
| Série | Série própria (padrão `1`), fora da faixa 70000 do Emissor Web |
| Competência | `dCompet` = data da emissão; o mês de referência vai só na descrição |
| Inscrição municipal | Configurável por ambiente (E0120 na Produção Restrita; E0116 exige em Produção em Catanduva) |
| Nota rejeitada | Refeita com o próximo número (Anexo I: sem inutilização; só E0014 trava número usado) |
| Cancelamento e reconciliação | Usam o ambiente gravado na DPS, não o ambiente atual da empresa |
| Exclusão de empresa | Sem notas: apaga; com notas: arquiva (documento fiscal) |
| E-mail | Conta SMTP do escritório; remetente com o nome da empresa e "responder para" o e-mail dela |

## Roadmap

**Fase 0 — Prova de conceito** · **concluída** (16/10/2026)

- Certificado A1, mTLS, XMLDSIG e leiaute da DPS aceitos pela Sefin; primeira NFS-e real autorizada e cancelada em Produção.
- Critério de continuidade atendido (em Produção, porque a Produção Restrita não tem o escritório cadastrado).

**Fase 1 — Uso diário no escritório** · **concluída no código** (16/10/2026), em validação no uso

1. Banco SQLite e cofre de certificados.
2. Emissão confiável, reconciliação e cancelamento.
3. Conferência da assinatura da Sefin e arquivamento em `<empresa>/<AAAA-MM>/`.
4. DANFSe v2.0 em PDF.
5. Aplicativo desktop (cadastros, emissão com revisão, notas, consulta de CNPJ, edição/exclusão de empresa, "Corrigir e emitir", "Consultar na Sefin").
6. Notas recorrentes com lote do mês e importação de planilha de convênios.
7. Envio por e-mail ao tomador.

**Fase 2 — Instalação e operação** · próxima

- Instalador MSI e CI que gera o instalador a cada versão; assinatura de código; atualização automática.
- Backup automático diário do banco, com teste de restauração.
- Log de erros exportável para suporte.
- Logomarca oficial da NFS-e no DANFSe; tabela de municípios IBGE (nome do município do tomador).
- Validação da cadeia ICP-Brasil do certificado; cache e fallback da consulta de CNPJ.

**Fase 3 — Pós-emissão e integração**

- Substituição de nota; consulta de eventos e sincronização periódica.
- Distribuição DF-e pelo ADN: baixar notas emitidas fora do sistema e notas recebidas como tomador; manifestação do tomador.
- Parâmetros municipais (alíquotas, serviços habilitados, prazos) para validar antes de enviar.
- Relatórios (faturamento, ISS devido/retido, IBS/CBS), ZIP mensal de XMLs para o Prosoft e painel do dia.
- Tomador estrangeiro, intermediário e classificação IBS/CBS na tela.

**Fora do escopo** (por decisão do escritório)

- Certificado A3; emissores municipais próprios; PostgreSQL e uso em rede; login, perfis e permissões por empresa; responder automaticamente aos e-mails dos convênios; Lucro Real.

## Fontes

- [Portal NFS-e — documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica)
- [Anexo I — leiaute e regras de negócio da DPS/NFS-e, v1.01-20260209 (gov.br)](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual)
- [NT 007 SE/CGNFS-e (gov.br)](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf)
- [LC 214/2025 e NFS-e nacional — Senior](https://documentacao.senior.com.br/exigenciaslegais/noticias/federal/2025/2025-02-20-nota-fiscal-de-servicos-eletronica-de-padrao-nacional-para-2026/)
- [NFS-e Nacional: o que muda — NotaGateway](https://notagateway.com.br/blog/tudo-sobre-nfs-e-nacional/)
- [Catanduva muda para o Emissor Nacional — NotaGateway](https://notagateway.com.br/blog/catanduva-sp-muda-emissao-da-nfs-e-para-o-emissor-nacional/)
- [Requisitos da API Sefin — NotaGateway](https://notagateway.com.br/?p=6164)
- [click-nfse (referência de implementação)](https://packagist.org/packages/noclick11/click-nfse)
- [brans-nfe (referência de implementação)](https://pypi.org/project/brans-nfe)
- [Odoo l10n\_br\_nfse\_nacional](https://pypi.org/project/odoo-addon-l10n-br-nfse-nacional/)
- [Eventos da NFS-e — Buscador NCM](https://buscadorncm.com.br/nfse/eventos)
- [Fim das atualizações ABRASF — Convergência Digital](https://convergenciadigital.com.br/mercado/associacao-das-secretarias-de-financas-decide-pelo-fim-das-atualizacao-da-nfs-e/)
- [Municípios não aderentes — Actana](https://actana.com.br/blog/quais-municipios-ja-aderiram-a-nfs-e-nacional-veja-o-mapa-atualizado)
