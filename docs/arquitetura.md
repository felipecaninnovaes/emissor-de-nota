# Arquitetura

Visão dos componentes, do fluxo de emissão e do armazenamento, para quem vai alterar o código. Regras fiscais ficam em [fiscal.md](fiscal.md); setup em [desenvolvimento.md](desenvolvimento.md).

## Crates

Workspace Cargo (`Cargo.toml`, membros `crates/*`). O núcleo fiscal não depende da interface.

| Crate | Papel |
| --- | --- |
| `nfse-core` | Domínio sem I/O: CNPJ (inclusive alfanumérico), CPF, Id da DPS, valores em centavos, modelo da DPS e validações do leiaute; Lista de Serviços Nacional embutida (`lista_servicos`) |
| `nfse-cert` | Certificado A1: leitura do `.pfx` (inclusive BER, em `ber.rs`), diagnóstico, assinatura RSA |
| `nfse-xml` | DPS e eventos na ordem do XSD, C14N inclusivo e exclusivo, assinatura e verificação XMLDSIG, leitura da NFS-e |
| `nfse-nacional` | Cliente mTLS da Sefin Nacional (trait `Sefin`), GZip + Base64, orientações por código de rejeição (`mensagens.rs`) |
| `nfse-db` | SQLite (sqlx): cofre, cadastros, numeração, DPS/NFS-e, recorrências, importações, e-mail |
| `nfse-emissor` | Orquestra emissão confiável, reconciliação, cancelamento, arquivamento, planilhas e e-mail |
| `nfse-pdf` | DANFSe v2.0 (NT 008/2026) com `krilla` e `qrcode` |
| `nfse-poc` | CLI de diagnóstico; ver [desenvolvimento.md](desenvolvimento.md#cli-nfse-poc) |
| `app` (`emissor-nfse`) | Aplicativo Tauri 2; interface em `crates/app/ui/` |

Dependências entre crates (conforme os `Cargo.toml`):

```
nfse-core
  ├─ nfse-cert ──┬─ nfse-xml
  │              ├─ nfse-nacional
  │              └─ nfse-db
nfse-pdf (independente)

nfse-emissor  → cert, core, db, nacional, pdf, xml
app           → cert, core, db, emissor, pdf, xml
nfse-poc      → cert, core, emissor, nacional, pdf, xml
```

## Fluxo de emissão

Implementado em `Emissor::emitir` (`crates/nfse-emissor/src/lib.rs`). A Sefin fica atrás dos traits `Conector` e `Sefin`, que os testes substituem por um simulado.

1. Recorrência e planilha: se o item já tem nota autorizada ou em andamento, não emite de novo.
2. Monta e **valida a DPS antes de reservar número**: DPS inválida não gasta numeração.
3. Reserva o número em transação atômica (`numeracao`, por empresa, ambiente e série) e grava a DPS como `enviando`.
4. Assina (XMLDSIG, em `spawn_blocking`) e envia (`POST /nfse`).
5. Resposta:
   - **autorizada**: grava a NFS-e exatamente como a Sefin devolveu (com SHA-256), confere a assinatura da Sefin (apenas aviso, ver [ADR 0007](decisoes/0007-assinatura-da-sefin-como-aviso.md)), gera o PDF e arquiva XML e PDF na pasta;
   - **rejeitada**: grava o erro; a DPS fica `rejeitada` e a correção usa o **próximo** número;
   - **sem resposta ou resposta ilegível** (timeout, 5xx): a DPS fica `incerta` e o emissor consulta `GET /dps/{id}` antes de qualquer reenvio.
6. `reconciliar` / `reconciliar_pendentes` resolvem as incertas (ao abrir o sistema e pelo botão "Verificar"): grava a NFS-e se existir ou libera a DPS para reenvio.

Situações da DPS (`SituacaoDps` em `nfse-db/src/dps.rs`): `reservada`, `enviando`, `autorizada`, `rejeitada`, `incerta`, `cancelada`, `substituida`.

Outras operações em `Emissor`: `cancelar` (evento e101101), `emitir_recorrencia`, `emitir_importado`, `ajustar_numeracao_pela_sefin`, `dados_para_refazer` (nota rejeitada), `dados_para_copiar` (autorizada ou cancelada), `enviar_email` e `enviar_email_apos_emissao`.

Descrição da nota: `aplicar_mes_ref` troca as variáveis (lista em [usuario.md](usuario.md#variáveis-na-descrição)); variável desconhecida é barrada antes do envio (`variaveis_desconhecidas`).

## Banco de dados

Um SQLite (SQLCipher) por máquina (`emissor.db` em `app_data_dir()`), em modo WAL com `synchronous = FULL` ([ADR 0008](decisoes/0008-synchronous-full.md)). Migrações em `crates/nfse-db/migrations/` (`0001` a `0006`), executadas pelo `sqlx::migrate!`. A recriação de tabela que exige desligar chaves estrangeiras roda como ajuste logo após as migrações (`crates/nfse-db/ajustes/`). As migrações têm de ficar em CRLF (`.gitattributes`); ver [desenvolvimento.md](desenvolvimento.md#migrações-do-banco).

| Tabela | Conteúdo |
| --- | --- |
| `pessoa` | Prestadores e tomadores: tipo, documento único, nome, endereço, IBGE, contato, fonte dos dados |
| `empresa` | Pessoa, inscrição municipal (e se vai na DPS, por ambiente), município emissor, regime, ambiente, série, ativa |
| `numeracao` | Próximo número por empresa, ambiente e série |
| `certificado` | `.pfx` e senha cifrados, thumbprint, titular, validade, ativo (histórico) |
| `empresa_tomador` | Vínculo empresa × tomador: e-mails de envio, observação, `retem_federais`, `retem_iss` |
| `servico` | Catálogo por empresa: descrição, códigos, tributação (JSON), valor sugerido |
| `dps` | Uma por tentativa: empresa, ambiente, série, número, Id (único por ambiente), situação, modelo, XML assinado, tentativas, erro |
| `nfse` | Documento fiscal: chave, número, XML autorizado e SHA-256, valor, competência |
| `evento` | Eventos da NFS-e (cancelamento) |
| `recorrencia`, `emissao_recorrente` | Modelo da mensalidade e a DPS de cada mês |
| `emissao_importada` | Chave de conteúdo da linha da planilha e a DPS; reimportar não duplica |
| `envio_email` | Registro de cada envio |
| `configuracao`, `auditoria` | Chaves da aplicação ([configuracao.md](configuracao.md)) e log de ações |

O ambiente é gravado como `producao` ou `homologacao` (`homologacao` é a Produção Restrita).

Regras: guardar sempre o XML autorizado original, com hash conferido na leitura; a versão do leiaute vai em cada DPS; o Id da DPS é único por ambiente.

## Cofre de certificados

`nfse-db/src/cofre.rs`: a chave AES-256 é derivada da senha mestra com Argon2id; o sal, os parâmetros e um verificador (sem segredo) ficam em `emissor.cofre`, ao lado do banco. Dessa chave sai, por SHA-256 com um rótulo, a chave do SQLCipher que cifra o `emissor.db` inteiro (`nfse-db/src/abertura.rs`; [ADR 0013](decisoes/0013-banco-cifrado-sqlcipher.md)). Por isso o `Banco` só existe depois da senha: `Estado.banco` começa vazio e o botão "Bloquear" fecha o banco. Um banco em claro de versão anterior é convertido na primeira abertura (o original fica como `emissor.db.legado`). Certificados e a senha do SMTP são cifrados com AES-256-GCM, com contexto por registro (`empresa_id:thumbprint:tipo`), e decifrados só em memória. A derivação roda em `spawn_blocking`. A senha mestra não tem recuperação. O botão "Bloquear" descarta a chave e volta à tela de senha.

## Backup cifrado

`nfse-db/src/backup.rs`: `VACUUM INTO` num arquivo temporário do sistema, cifrado em blocos de 1 MiB com a chave do cofre (AES-256-GCM; o AAD amarra o índice do bloco e se é o último, o que detecta truncamento e reordenação) e gravado como `emissor-AAAAMMDD-HHMMSS.nfbk`. O cabeçalho leva o sal e os parâmetros do Argon2 (sem segredo), então a senha mestra abre o backup em outra máquina. O banco já guarda os XMLs autorizados e os eventos, por isso o backup é só o banco. `restaurar_backup` decifra para um arquivo novo, grava o `.cofre` ao lado e roda `PRAGMA integrity_check`. Comandos em `crates/app/src/comandos/backup.rs`; o automático roda ao abrir o cofre (a cada 20 h, no máximo) e falha só na auditoria.

## Aplicativo (Tauri)

- `crates/app/src/main.rs`: abre o banco, registra os plugins (`updater`, `window-state`) e o `generate_handler!`.
- `crates/app/src/comandos/`: comandos IPC, um módulo por assunto (`aparencia`, `atualizacao`, `cofre`, `consultas`, `email`, `emissao`, `empresas`, `notas`, `planilha`, `recorrencias`, `servicos`, `tomadores`); `comum.rs` guarda o que é compartilhado (`exigir_cofre`, macro `emissor!`, `abrir_no_sistema`). Todo comando que lê ou grava dados exige o cofre aberto.
- `crates/app/src/cnpj.rs` (BrasilAPI) e `cep.rs` (ViaCEP): consultas públicas; só o CNPJ ou o CEP saem da máquina.
- `crates/app/ui/`: `index.html`, `app.js`, `estilo.css` e fontes IBM Plex (OFL); sem Node nem bundler ([ADR 0001](decisoes/0001-interface-sem-bundler.md)). CSP `default-src 'self'` em `tauri.conf.json`.
- Erros dos comandos voltam como `String` pronta para o operador.

## Arquivos em disco

- Banco: `app_data_dir()/emissor.db` (identificador `br.emissor-nfse.app`; no macOS a pasta é `br.emissor-nfse`, [ADR 0014](decisoes/0014-pasta-de-dados-no-macos.md)).
- Notas: `<pasta de saída>/<EMPRESA - CNPJ>/<AAAA-MM>/NFSe-<número>-<chave>.xml` (e o PDF), em `crates/nfse-emissor/src/arquivo.rs`. O XML de evento ganha o sufixo do evento.
