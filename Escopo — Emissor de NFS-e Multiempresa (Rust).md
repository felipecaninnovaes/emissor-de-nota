# Escopo — Emissor de NFS-e Multiempresa (Rust)

Oct 16, 2026 · @Felipe Canin Novaes

## Visão geral e objetivos

O produto é um emissor desktop de NFS-e, operado pelo escritório de contabilidade em nome de dezenas ou centenas de empresas clientes, cada uma com seu próprio certificado digital, cadastro de tomadores e catálogo de serviços. O núcleo de integração deve ser a **NFS-e Padrão Nacional (API Sefin Nacional / ADN)**; integrações com emissores municipais próprios entram como módulos opcionais.

**Atores**

- **Operador do escritório**: emite, cancela, substitui e consulta notas para várias empresas no mesmo dia.
- **Administrador do escritório**: cadastra empresas, certificados, usuários e permissões.
- **Empresa cliente (prestador)**: dona do CNPJ e do certificado; pode precisar receber relatórios ou o PDF da nota, mas não opera o sistema na versão inicial.
- **Tomador**: quem recebe a nota (PJ via CNPJ, PF via CPF, ou estrangeiro).

**Objetivos mensuráveis a definir com o escritório**

- Tempo para emitir uma nota recorrente (meta sugerida: menos de 30 segundos com tomador e serviço já cadastrados).
- Volume mensal de notas e número de empresas ativas (dimensiona SQLite vs PostgreSQL).
- Quantos usuários simultâneos (1 máquina ou rede local com vários postos).
- Zero nota duplicada e zero nota perdida em queda de conexão.

## Cenário regulatório (out/2026)

Desde 1º/01/2026 todo município precisa operar no padrão nacional, mas **pode emitir pelo Emissor Nacional ou manter emissor próprio e só compartilhar as notas com o ADN** ([LC 214/2025, art. 62 — Senior](https://documentacao.senior.com.br/exigenciaslegais/noticias/federal/2025/2025-02-20-nota-fiscal-de-servicos-eletronica-de-padrao-nacional-para-2026/); [NotaGateway](https://notagateway.com.br/blog/tudo-sobre-nfs-e-nacional/)). Isso define a arquitetura: o software precisa saber, por município, **onde** a nota é autorizada.

| Situação do município | Onde o software emite | Exemplos |
| --- | --- | --- |
| Usa o Emissor Nacional | API Sefin Nacional (padrão único) | Catanduva/SP desde 01/09/2026 ([NotaGateway](https://notagateway.com.br/blog/catanduva-sp-muda-emissao-da-nfs-e-para-o-emissor-nacional/)) |
| Emissor próprio, layout nacional | Webservice da prefeitura (layout próximo ao nacional) | Teresópolis/RJ ([comunicado](https://www.teresopolis.rj.gov.br/wp-content/uploads/2025/12/Comunicado-Nota-Padrao-Nacional.pdf)) |
| Emissor próprio, layout municipal | Webservice próprio da prefeitura | São Paulo capital e DF ([Contábeis](https://www.contabeis.com.br/forum/tributos-estaduais-municipais/412710/nota-fiscal-eletronica-municipal-e-nfse/); [Actana](https://actana.com.br/blog/quais-municipios-ja-aderiram-a-nfs-e-nacional-veja-o-mapa-atualizado)) |
| MEI | Sempre Emissor Nacional (obrigatório desde 2023) | Qualquer município |

**Pontos que mudam o escopo**

- **Catanduva** (sede provável do escritório) migrou para o Emissor Nacional em 01/09/2026; guias de ISSQN e obrigações acessórias continuam no sistema municipal ([NotaGateway](https://notagateway.com.br/blog/catanduva-sp-muda-emissao-da-nfs-e-para-o-emissor-nacional/)). Se a maioria dos clientes está na região, a API nacional cobre quase tudo.
- **Modelo ABRASF** não recebe mais atualizações ([Convergência Digital](https://convergenciadigital.com.br/mercado/associacao-das-secretarias-de-financas-decide-pelo-fim-das-atualizacao-da-nfs-e/)); integrações municipais devem ser tratadas como legado.
- **Reforma Tributária (IBS/CBS)**: o layout ganhou o grupo `IBSCBS` na DPS e totalizadores na NFS-e, com sucessivas Notas Técnicas (NT 004, 005, 007, 009) e o campo `cIndOp` (indicador de operação) ([NT 007 — gov.br](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf); [Jornal Contábil — NT 009](https://jornalcontabil.com.br/noticia/nfs-e-publicadas-atualizacoes-sobre-o-layout-do-padrao-nacional-que-inclui-regras-para-ibs-e-cbs/)). ISS, IBS e CBS convivem até 2032, então o layout continuará mudando: **versionar o leiaute e os XSDs é requisito, não detalhe**.
- **Prazos de cancelamento e substituição** são parametrizados por cada município no Sistema Nacional; não há prazo único. A substituição fora do prazo só é aceita para enquadramento/desenquadramento no Simples Nacional ([Buscador NCM](https://buscadorncm.com.br/nfse/eventos)).
- **Código de serviço**: o padrão nacional usa `cTribNac` de 6 dígitos sem pontos (ex.: 010101), derivado da lista da LC 116 ([Teresópolis](https://www.teresopolis.rj.gov.br/wp-content/uploads/2025/12/Comunicado-Nota-Padrao-Nacional.pdf)); a NBS e o código municipal podem ser exigidos conforme parametrização.

**A levantar com o escritório**: lista de municípios dos clientes e, para cada um, se usa o Emissor Nacional; regimes (MEI, Simples, Presumido, Real); quantos têm retenções (ISS retido, PIS/COFINS/CSLL, IR, INSS).

## Integração técnica — API Sefin Nacional / ADN

O software não envia uma "nota": envia uma **DPS (Declaração de Prestação de Serviço)** assinada; a Sefin Nacional valida, calcula os tributos, gera a NFS-e em XML com a DPS encapsulada e devolve uma chave de acesso de 50 dígitos ([NotaGateway — NT 004](https://notagateway.com.br/?p=5546); [click-nfse](https://packagist.org/packages/noclick11/click-nfse)).

**Requisitos de comunicação**

- **mTLS** com certificado ICP-Brasil A1 ou A3 do prestador, com EKU "Client Authentication" ([NotaGateway](https://notagateway.com.br/?p=6164)).
- **Assinatura XMLDSIG** da DPS e dos eventos: C14N inclusiva, RSA-SHA1, só o certificado final no KeyInfo, sem prefixo `ds:` ([click-nfse](https://packagist.org/packages/noclick11/click-nfse)). Prefixo de namespace gera rejeição E1228 ([fórum ACBr](https://www.projetoacbr.com.br/forum/topic/92867-nfs-e-nacional-sefin-produ%C3%A7%C3%A3o-restrita-%E2%80%94-e999-mesmo-com-xmldsig-ok-e-dps-equivalente-ao-emissor-web/)).
- **Transporte**: corpo JSON com o XML compactado em GZip e codificado em Base64.
- **Ordem dos elementos**: a DPS precisa seguir exatamente o `xs:sequence` do XSD; validar localmente contra o XSD antes de enviar.
- **Ambientes**: Produção Restrita (homologação) e Produção, com URLs e certificados de teste separados por empresa.

**Operações a implementar**

| Operação | Endpoint (referência) | Observação |
| --- | --- | --- |
| Emitir | `POST /nfse` | Síncrono; retorna chave, número e XML autorizado |
| Consultar NFS-e | `GET /nfse/{chaveAcesso}` | XML GZip+Base64 |
| Consultar DPS | `GET /dps/{id}` · `HEAD /dps/{id}` | Base da reconciliação após timeout |
| Registrar evento | `POST /nfse/{chave}/eventos` | Cancelamento (e101101), substituição, manifestação |
| Consultar eventos | `GET /nfse/{chave}/eventos` | Situação atual da nota |
| Distribuição DF-e | ADN, paginado por NSU | Notas recebidas como tomador e emitidas por terceiros |
| Parâmetros municipais | API de parametrização | Alíquotas, serviços aceitos, prazos de cancelamento |

Endpoints conforme as bibliotecas de referência ([brans-nfe](https://pypi.org/project/brans-nfe); [NotaGateway](https://notagateway.com.br/?p=6164)); confirmar no Manual de Integração oficial do [Portal NFS-e](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica).

**Regras críticas de negócio**

- **Idempotência**: o Id da DPS (CNPJ + série + número) é a chave de idempotência. Em timeout, consultar `GET /dps/{id}` antes de reenviar; nunca reenviar às cegas ([click-nfse](https://packagist.org/packages/noclick11/click-nfse); [Odoo l10n\_br\_nfse\_nacional](https://pypi.org/project/odoo-addon-l10n-br-nfse-nacional/)).
- **Numeração**: a série e o número da DPS são do emitente e não há inutilização; o número da NFS-e é reservado pela Sefin e pode ter "pulos" ([NotaGateway — NT 007](https://notagateway.com.br/?p=10731)).
- **DANFSe**: o PDF é gerado localmente a partir do XML autorizado (layout v2.0 da NT 008/2026) ([Odoo l10n\_br\_nfse\_nacional](https://pypi.org/project/odoo-addon-l10n-br-nfse-nacional/)).
- **Erros**: mapear o catálogo de códigos (E0717 assinatura, E1235 Id inválido, E2404 DPS sem NFS-e etc.) para mensagens em português simples.
- **Contingência**: sem contingência offline como na NF-e; com a Sefin fora, a nota fica em fila e é reenviada com reconciliação.
- **Rate limit (429)** e retry com backoff apenas em consultas.

## Certificado digital por empresa

Cada empresa tem um e-CNPJ, e ele serve duas vezes: assina a DPS/eventos e autentica a conexão mTLS. Para um escritório que opera dezenas de CNPJs, **o A1 (.pfx) é o caminho prático**; o A3 (token/cartão) exige o dispositivo físico na máquina e só funciona via PKCS#11 ou repositório do Windows.

**Requisitos**

- Importar .pfx/.p12 com senha; extrair CNPJ, razão social, validade e emissora; **bloquear vínculo se o CNPJ do certificado não bater com o da empresa**.
- Guardar o .pfx **criptografado no banco** (AES-256-GCM) com chave protegida pelo Windows DPAPI ou por uma senha mestra do escritório; nunca gravar a senha do certificado em texto plano.
- Alternativa no Windows: usar o certificado instalado no repositório do usuário (CryptoAPI/CNG), útil para A3.
- Validar a cadeia ICP-Brasil (embarcar o bundle de ACs e permitir atualização).
- Verificar se o EKU contém Client Authentication (OID 1.3.6.1.5.5.7.3.2).
- **Painel de vencimentos**: alerta 30/15/7 dias antes, por empresa, com relatório para o escritório cobrar a renovação.
- Histórico de certificados (troca sem perder notas antigas).
- Procuração eletrônica: confirmar se o escritório pode usar o próprio e-CNPJ com procuração em vez do certificado de cada cliente — **não há indicação de que a API Sefin aceite isso hoje**; tratar como pergunta em aberto.

**Risco conhecido**: há relatos de rejeição (E0717) com certificados "RFB e-CNPJ A1" de algumas ACs em eventos de manifestação, mesmo válidos ([fórum ACBr](https://www.projetoacbr.com.br/forum/topic/91333-erro-e0717-ao-registrar-evento-de-manifesta%C3%A7%C3%A3o-de-tomador-%E2%80%94-certificado-icp-brasil-e-cnpj-rfb-a1-rejeitado-pelo-webservice/)). Ter um diagnóstico de certificado embutido (validade, titular, EKU, AC) ajuda o suporte.

## Requisitos funcionais por módulo

**1. Empresas (prestadores)**

- CNPJ com consulta pública (razão social, fantasia, endereço, CNAE, situação cadastral, opção Simples/MEI).
- Inscrição municipal, município (código IBGE), regime tributário, regime especial de tributação, optante Simples (e se apura IBS/CBS por fora), incentivos fiscais.
- Série e próximo número de DPS por empresa, ambiente (homologação/produção), certificado ativo.
- Configuração de retenções padrão e textos complementares fixos.
- Logotipo para o DANFSe e e-mail remetente.

**2. Tomadores (clientes das empresas)**

- **Cadastro global reutilizável**: um tomador (ex.: CNPJ X) existe uma vez na base e é vinculado a várias empresas; dados específicos da relação (e-mail de envio, condição de pagamento, observação) ficam no vínculo.
- **PJ**: consulta pública de CNPJ com cache local e botão "atualizar dados"; provedores gratuitos como BrasilAPI, ReceitaWS, CNPJ.ws e CNPJá têm limites de poucas requisições por minuto, então usar fallback entre provedores e fila ([get-cnpj](https://packagist.org/packages/azumamagus/get-cnpj); [cnpj-ws](https://github.com/jos3duardo/cnpj-ws)).
- **PF**: CPF validado só pelo dígito verificador; nome, endereço e e-mail digitados (não existe consulta pública de CPF); CEP com autocompletar.
- **Estrangeiro**: NIF ou motivo de ausência, país, endereço no exterior.
- **Tomador não identificado**: alguns municípios permitem; verificar regras de cancelamento/substituição, que podem ser bloqueados nesse caso ([Santa Cruz do Sul](https://www.santacruz.rs.gov.br/decretos/docs/DECRETOS_2025/DECRETO_12656_regulamenta_emissao_NFS-e_padrao_nacional.pdf)).
- Intermediário do serviço (opcional na DPS).

**3. Catálogo de serviços por empresa**

- Descrição padrão, `cTribNac` (6 dígitos), NBS, código municipal, CNAE, local de incidência, alíquota de ISS, retenções (ISS, PIS, COFINS, CSLL, IR, INSS), classificação IBS/CBS (CST/cClassTrib, `cIndOp`) e valor sugerido.
- Importar a tabela nacional de códigos e validar contra a parametrização do município.

**4. Emissão**

- Fluxo rápido: empresa → tomador (busca por nome/documento) → serviço(s) → valor → revisar → emitir.
- Pré-visualização dos tributos antes de enviar; validação XSD local.
- **Duplicar nota anterior** e **notas recorrentes** (mensalidades) com agendamento/lote.
- Emissão em lote (planilha CSV/XLSX importada).
- Competência diferente da data de emissão.
- Envio automático por e-mail (PDF + XML) ao tomador.

**5. Pós-emissão (eventos)**

- Cancelamento com motivo; substituição (gera nova DPS vinculada); respeitar prazos por município.
- Consulta de situação e sincronização periódica.
- Download do XML/DANFSe de notas emitidas fora do sistema (pelo portal), via ADN.
- Manifestação do tomador (confirmar/rejeitar) quando a empresa for tomadora.

**6. Relatórios e integração contábil**

- Notas por empresa/período, faturamento, ISS devido e retido, IBS/CBS destacados.
- Exportação para o sistema contábil/fiscal do escritório (definir qual: Domínio, Alterdata, Questor, SCI etc.) e pacote ZIP de XMLs mensal.
- Painel do dia: notas pendentes, rejeitadas, certificados vencendo.

**7. Usuários e auditoria**

- Login, perfis (admin, operador, consulta) e restrição por empresa.
- Log de auditoria de quem emitiu/cancelou e quando.

## Modelo de dados (entidades principais)

O ponto central é separar **pessoa** (dado público, único por documento) de **vínculo** (relação de uma empresa com aquele tomador). Isso atende o requisito de reutilizar tomadores já cadastrados.

| Entidade | Campos-chave | Relações |
| --- | --- | --- |
| `pessoa` | tipo (PJ/PF/EXT), documento único, nome, endereço, CEP, cód. IBGE, e-mail, fonte dos dados, atualizado\_em | Base de prestadores e tomadores |
| `empresa` | pessoa\_id, IM, regime, Simples/MEI, série DPS, próximo nº, ambiente, ativa | 1 pessoa · N certificados · N serviços |
| `certificado` | empresa\_id, pfx criptografado, thumbprint, validade, AC, ativo | N por empresa (histórico) |
| `empresa_tomador` | empresa\_id, pessoa\_id, e-mails de envio, observações | Vínculo reutilizável |
| `servico` | empresa\_id, descrição, cTribNac, NBS, cód. municipal, alíquotas, retenções, cClassTrib, cIndOp | N por empresa |
| `municipio` | cód. IBGE, UF, modo (nacional/próprio), provedor, parâmetros em cache | Decide o adaptador de envio |
| `dps` | empresa\_id, série, número, Id DPS, status, XML enviado, tentativas | 1 DPS → 0..1 NFS-e |
| `nfse` | dps\_id, chave 50 díg., número, XML autorizado, valores, situação | N eventos |
| `evento` | nfse\_id, tipo, protocolo, XML, data | Cancelamento, substituição, manifestação |
| `fila_envio` | dps\_id, próxima tentativa, último erro | Reenvio com reconciliação |
| `usuario` · `permissao` · `auditoria` | perfil, empresas permitidas, ação, data, IP/máquina | Controle de acesso |

**Status da DPS**: rascunho → validada → enviando → autorizada / rejeitada / incerta (timeout, aguardando reconciliação) → cancelada / substituída.

**Regras**

- Guardar **sempre o XML autorizado original** (é o documento fiscal); valores em colunas só para relatório.
- Número da DPS reservado em transação atômica (evita dois operadores usarem o mesmo número).
- Versão do leiaute gravada em cada DPS, para reprocessar/exibir notas antigas.
- Migrações versionadas (ex.: `sqlx migrate` ou `refinery`) que rodem em SQLite e PostgreSQL.

## Arquitetura e stack Rust

Recomendação: um workspace Cargo com o **núcleo fiscal isolado da interface**, e cada destino de emissão como um adaptador atrás de um trait comum (`EmissorNfse`). Assim o padrão nacional é o primeiro adaptador, e São Paulo ou outro emissor próprio podem entrar depois sem mexer no restante.

**Crates do workspace (sugestão)**

- `nfse-core`: tipos de domínio, regras de cálculo e validação, sem I/O.
- `nfse-xml`: serialização da DPS/eventos na ordem do XSD, validação XSD, C14N e assinatura.
- `nfse-nacional`: cliente mTLS da Sefin/ADN, reconciliação, distribuição DF-e.
- `nfse-db`: repositórios e migrações (SQLite e PostgreSQL).
- `nfse-pdf`: DANFSe a partir do XML autorizado.
- `app`: interface desktop, fila de envio, agendamentos.

**Bibliotecas candidatas**

| Necessidade | Opções | Atenção |
| --- | --- | --- |
| Interface desktop | Tauri 2 (WebView2 no Windows), Slint, egui/iced | Tauri dá UI web rica e instalador MSI; Slint é nativo e leve |
| Banco | `sqlx` (SQLite + Postgres, queries verificadas em compilação) ou SeaORM | Escrever SQL compatível com os dois ou abstrair |
| HTTP + mTLS | `reqwest` com `native-tls` (Identity a partir de PKCS#12) ou `rustls` | Testar cedo com certificado real em Produção Restrita |
| XML | `quick-xml` (+ `serde`) | Ordem dos elementos manda; testes de snapshot contra XML do Emissor Web |
| Validação XSD | `libxml` (bindings da libxml2) | Dependência C no Windows; empacotar a DLL |
| Assinatura XMLDSIG | Não há crate Rust madura; implementar C14N + `rsa` + `sha1` + `pkcs12`/`p12`, ou bindings do xmlsec1 | **Maior risco técnico do projeto**: fazer prova de conceito na semana 1 |
| A3 / repositório Windows | crate `windows` (CNG/CryptoAPI), `cryptoki` (PKCS#11) | Só se A3 entrar no escopo |
| Criptografia em repouso | `aes-gcm`, `argon2`, DPAPI via `windows` | Chave nunca no código |
| PDF | Typst embutido, `printpdf`/`genpdf` | Layout v2.0 do DANFSe com QR Code |
| E-mail | `lettre` | SMTP do escritório |
| Planilhas | `calamine` (leitura), `rust_xlsxwriter` | Importação em lote |
| Assíncrono e logs | `tokio`, `tracing` | Logs sem dados sensíveis |

**SQLite vs PostgreSQL**

- **SQLite** basta para 1 posto ou poucos usuários na mesma máquina; backup é copiar um arquivo. Não usar SQLite em pasta de rede compartilhada (risco de corrupção).
- **PostgreSQL** se houver vários operadores em máquinas diferentes na rede do escritório.
- Decisão prática: começar com SQLite, mas manter o código compatível com os dois desde o início.

**Distribuição**: instalador MSI assinado (code signing evita alerta do SmartScreen), atualização automática, e embarque das ACs ICP-Brasil e XSDs versionados.

## Segurança, LGPD, backup e guarda

O banco concentra os certificados de todos os clientes do escritório: quem copia o arquivo do banco e descobre a chave pode emitir notas em nome de qualquer empresa. A segurança dos certificados é o requisito não funcional mais importante.

- **Certificados**: criptografados em repouso; chave derivada de senha mestra (Argon2) ou protegida por DPAPI; descriptografar só em memória no momento do uso e zerar depois (`zeroize`).
- **Acesso**: login obrigatório, senha com hash Argon2, bloqueio após tentativas, perfis por empresa.
- **LGPD**: CPF, nome e endereço de pessoas físicas são dados pessoais. Definir base legal (obrigação legal/fiscal), registrar finalidade, permitir exportar/anonimizar dados de tomador PF que não tenha notas, e não enviar dados a serviços de terceiros além dos fiscais.
- **Consulta de CNPJ**: só o CNPJ sai para a API pública; registrar a fonte e a data da consulta.
- **Logs**: nunca registrar senha, chave privada ou XML completo com dados pessoais em log de depuração.
- **Backup**: rotina automática diária (cópia do SQLite ou `pg_dump`), com retenção e teste de restauração; opção de destino externo (pasta de rede, nuvem do escritório).
- **Guarda de XML**: manter os XMLs autorizados pelo prazo decadencial/prescricional (no mínimo 5 anos; confirmar com o contador a regra adotada pelo escritório), com exportação em ZIP por empresa/mês.
- **Integridade**: hash do XML autorizado gravado junto, para detectar alteração.

## Riscos, perguntas em aberto e roadmap

**Riscos**

| Risco | Impacto | Mitigação |
| --- | --- | --- |
| Assinatura XMLDSIG em Rust sem crate madura | Bloqueia a emissão | Prova de conceito na primeira semana com certificado real em Produção Restrita |
| Layout muda a cada Nota Técnica (IBS/CBS até 2032) | Rejeições após atualização da Sefin | XSDs versionados, testes automatizados, acompanhar o portal NFS-e |
| Clientes em municípios com emissor próprio | Escopo multiplica | Levantar municípios antes; adaptadores só sob demanda |
| Timeout na emissão gera nota duplicada | Duplicidade fiscal | Reconciliação por Id da DPS antes de qualquer reenvio |
| Vazamento do banco com certificados | Emissão fraudulenta | Criptografia forte, senha mestra, controle de acesso |
| Limites das APIs públicas de CNPJ | Cadastro lento | Cache local, fallback entre provedores |
| Parametrização municipal divergente (alíquota, serviço não habilitado) | Rejeição | Consultar parâmetros municipais e validar antes de enviar |

**Perguntas para o escritório**

- [ ] Quantas empresas, quantas notas por mês e quantos operadores simultâneos? 35
- [ ] Em quais municípios estão os clientes, e quais usam o Emissor Nacional? Principalmente Catanduva, mas em Novembro/2026 todos os municípios deveram estar homologados no ambiente nacional.
- [ ] Algum cliente com certificado A3? Todos já têm e-CNPJ? 90% já tem E-CNPJ, os que não tiverem vai ser solicitado para fazer.
- [ ] Quais regimes e retenções são comuns (ISS retido, INSS, IR, PIS/COFINS/CSLL)? ISS RETIDO, IRRF, CSLL, PIS, COFINS.
- [ ] Há notas recorrentes (mensalidades) e emissão em lote? Sim, o proprio escritorio tem as mensalidades que não mudam os valores apenas na descrição o mês que refere o serviço, e muitos dos clientes/empresas são clinicas, tem bastante notas de convenio.
- [ ] Qual sistema contábil/fiscal recebe os dados e em que formato? Eles usam prosoft que importa os xmls, não tem nenhum sistema de emissão atualmente.
- [ ] O cliente final precisa acessar algo (portal, PDF por e-mail)? Boa parte é por e-mail, mas vai ser necessário tem uma pasta separado por empresas em meses xml e pdf pois alguns convênios obriga que seja respondido o e-mail.
- [ ] Rede local com servidor ou máquinas isoladas (define SQLite x PostgreSQL)? Até tem servidor mas é mais complexo e certas empresas outros usuarios não podem ter acesso, então o foco será por maquina
- [ ] Quem dá suporte e como as atualizações chegam às máquinas? Eu mesmo
- [ ] Modelo comercial: uso interno ou produto para vender a outros escritórios (muda licenciamento, multi-tenant e homologação)? Uso interno mas o repositório do projeto vai ser publico.

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

**Ainda em aberto**

- [ ] Quantas notas por mês no total e quantos operadores (máquinas)? Media 500 notas
- [ ] Regimes das empresas (Simples, Presumido, Real)? Define cálculo de ISS, retenções e IBS/CBS. Simples e presumido.
- [ ] O que é "responder o e-mail" exigido por alguns convênios: enviar a nota como resposta ao e-mail recebido do convênio (exige ler a caixa de entrada) ou só enviar para um endereço específico? De inicio isso ficara manual para o usuário, não vamos inflar o escopo.
- [ ] Qual layout o Prosoft espera na importação (XML da NFS-e nacional puro ou outro formato)? XML Puro
- [ ] Usuários diferentes na mesma máquina precisam de login separado, ou cada máquina é de um operador? Cada maquina é um operador.

**Decisões complementares**

| Tema | Decisão | Impacto no escopo |
| --- | --- | --- |
| Volume | Média de 500 notas/mês (cerca de 25 por dia útil) | SQLite local sobra; o gargalo é o tempo de digitação, então recorrentes e lote valem mais que desempenho |
| Regimes | Simples Nacional e Lucro Presumido | Lucro Real fora do escopo. Simples: informar optante e alíquota da faixa, sem destacar PIS/COFINS próprios. Presumido: ISS pela alíquota municipal e retenções federais configuráveis por serviço |
| E-mail dos convênios | Manual pelo usuário por enquanto | Fora do escopo; o sistema só gera os arquivos na pasta e envia o e-mail simples ao tomador |
| Prosoft | Importa o XML puro | Exportação = cópia do XML autorizado, sem conversão |
| Usuários | Cada máquina é de um operador | Sem perfis nem permissões por empresa; fica só uma senha local para abrir o app e proteger os certificados. O módulo 7 se reduz a log de auditoria |

**Roadmap**

&#91;embedded content: roadmap · 4 fases\]

A Fase 0 vale como critério de continuidade: se a assinatura e o mTLS não funcionarem em Produção Restrita, o restante não importa. As durações dependem do tamanho da equipe e ficam para depois das respostas acima.

## Fontes

- [Portal NFS-e — documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica)
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
