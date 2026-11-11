# AGENTS.md

Instruções para agentes de código (e pessoas) que alteram este repositório. Detalhes ficam em [docs/](docs/); aqui só o que evita erro.

## O projeto

Emissor desktop de NFS-e Padrão Nacional, em Rust: workspace `crates/*` com o núcleo fiscal (`nfse-core`, `nfse-cert`, `nfse-xml`, `nfse-nacional`, `nfse-db`, `nfse-emissor`, `nfse-pdf`), a CLI `nfse-poc` e o app Tauri 2 (`crates/app`, pacote `emissor-nfse`) com interface em HTML/CSS/JS sem bundler. Mapa dos crates e fluxo de emissão: [docs/arquitetura.md](docs/arquitetura.md).

## Comandos

O `justfile` é o contrato de comandos (receitas em PowerShell no Windows):

```bash
just setup            # git hooks (pre-commit) e cargo fetch
just run              # cargo run -p emissor-nfse
just check            # fmt-check + clippy -D warnings + testes do workspace
just test -p nfse-xml # ou um filtro: just test nome_do_teste
just fmt
just check-servidor   # servidor/, precisa do Postgres do compose.yaml
```

No Linux e no macOS, o ambiente vem do Devbox (`devbox run setup`, `up`, `test`, `lint`, `fmt`, `run`, `down`). O CI roda fmt, clippy e testes no Windows. No Linux, `nfse-nacional::testes::identidade_mtls_a_partir_do_pfx` falha porque o OpenSSL 3 não abre o `.pfx` legado de teste; os outros passam, e `just check` e `devbox run test` o pulam fora do Windows ([docs/desenvolvimento.md](docs/desenvolvimento.md#problemas-conhecidos)).

## Regras

- **O código é a fonte da verdade.** Se um documento divergir, corrija o documento. Ao mudar comportamento, atualize o documento dono do assunto (lista em [README.md](README.md#documentação)).
- **Repositório público:** nunca versionar certificados reais, senhas, bancos, XMLs ou PDFs de clientes. Nunca registrar senha, chave privada ou XML completo em log.
- **Dados de clientes ou do escritório** (nomes, volumes, municípios) não entram na documentação.
- **Migrações** (`crates/nfse-db/migrations/`): arquivos novos em CRLF, com o número seguinte ao do último arquivo da pasta. Nunca editar uma migração já aplicada (o sqlx confere o checksum). Coluna ou índice novo na tabela `dps` não vai em migração, e sim em `crates/nfse-db/src/ajustes.rs` (idempotente, depois de `dps_unica_por_ambiente`, que recria a tabela num banco novo). Ver [docs/desenvolvimento.md](docs/desenvolvimento.md#migrações-do-banco).
- **Comandos Tauri:** todo `#[tauri::command]` novo vai em `crates/app/src/comandos/<assunto>.rs`, entra no `generate_handler!` de `main.rs` com o caminho completo (`pub use` não funciona) e, se lê ou grava dados, chama `exigir_cofre(&estado)` ou usa a macro `emissor!`. Esquecer o registro compila e quebra a tela.
- **Emissão:** a DPS é validada antes de reservar número; a DPS é gravada como `enviando` antes do envio; em resposta incerta, consultar a Sefin antes de reenviar. Não quebre essas garantias ([docs/fiscal.md](docs/fiscal.md)).
- **Produção:** o agente nunca emite, cancela nem registra evento em Produção (pela CLI `nfse-poc` ou pelo app). Mostra a DPS ou o comando para conferência e o usuário executa. Na Produção Restrita, só com pedido explícito.
- **Mudança fiscal pronta:** alteração em `nfse-xml`, `nfse-emissor` ou na montagem da DPS só termina com teste que valide o XML no XSD de cada leiaute afetado (NT 004 e NT 009, com `xmllint`, como os testes existentes) e com [docs/fiscal.md](docs/fiscal.md) atualizado quando a regra muda. Na mensagem de commit, cite o código de rejeição da Sefin que motivou a mudança, se houver.
- **Decisões já tomadas** que não devem ser revertidas sem conversa: [docs/decisoes/](docs/decisoes/) (por exemplo, `synchronous = FULL`, assinatura da Sefin só como aviso, interface sem bundler).
- **Interface:** argumentos de primeiro nível do `invoke` em camelCase; sem estilo em linha; erros de validação junto do campo.
- **Idioma:** código, comentários e mensagens em português do Brasil, no estilo do código vizinho.
- **Branches:** nada de commit direto na `main` (o hook do pre-commit e o do Claude Code bloqueiam). Trabalho em branch curta a partir da `main` (`feat/`, `fix/`, `refactor/`, `docs/`, `chore/`, nome em português, ex.: `fix/u-fffe-na-assinatura`) e entra por PR com CI verde, com merge **sem squash** (rebase), para cada etapa continuar sendo um commit no histórico.
- **Commits:** toda alteração concluída vira commit logo em seguida, um commit por alteração; não acumule mudanças sem commit. Antes de commitar: `just fmt`, clippy sem avisos e os testes dos crates alterados.
  - Adicione só os arquivos que você alterou, pelo nome (`git add <arquivo>`); nunca `git add -A` nem `git add .`, para não levar mudanças que já estavam pendentes na árvore.
  - Mensagem no formato Conventional Commits com o título em português: `tipo(escopo): o que mudou`, com tipo `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci` ou `chore` (`!` depois do tipo = quebra de compatibilidade) e escopo opcional com o nome curto do crate ou da área (`emissor`, `xml`, `db`, `app`, `servidor`). Ex.: `fix(emissor): recusa U+FFFE antes de reservar número`. No corpo, o motivo, o código de rejeição da Sefin ou a issue (`issue #N`) e o que ficou de fora. O hook `commit-msg` confere o formato.
  - Learning do trail para a equipe (`.trail/learnings/`) entra no mesmo commit da alteração a que se refere.
  - Commit sim, `git push` não: enviar a branch ou a tag só com confirmação.
- **Changelog:** commit que muda algo visível para quem usa o aplicativo (tela, nota emitida, regra fiscal, instalação) acrescenta uma linha na seção `## Não publicado`, no topo do [CHANGELOG.md](CHANGELOG.md), no mesmo commit e no formato das seções existentes. Refatoração, teste e documentação interna não entram.
- **Número da versão** (semver): só correção, sem comportamento novo → patch (`1.8.1` → `1.8.2`); funcionalidade nova ou mudança de leiaute ou de regra fiscal → minor (`1.8.1` → `1.9.0`); migração de banco → no mínimo minor. Major só por decisão do usuário.
- **Versão e release:** a tag `vX.Y.Z` precisa bater com `version` em `crates/app/Cargo.toml` (e o `Cargo.lock`); ver [docs/deploy.md](docs/deploy.md). Só faça o bump e a tag quando pedido, e então: (1) suba a `version` e atualize o `Cargo.lock`; (2) no [CHANGELOG.md](CHANGELOG.md), renomeie `## Não publicado` para `## X.Y.Z`, confira com `git log vANTERIOR..HEAD` se falta alguma alteração, escreva a frase de resumo da versão (com a menção à mudança de esquema do banco), acrescente a data na frase de abertura e deixe uma seção `## Não publicado` vazia no topo; (3) commit `chore(release): versão X.Y.Z` numa branch `chore/versao-X.Y.Z`, com PR; (4) depois do merge, na `main` atualizada, tag anotada com a descrição das alterações (`git tag -a vX.Y.Z -F <arquivo>`, título `Versão X.Y.Z` e, abaixo, o texto da seção do changelog). Envie a tag só com confirmação.

## Convenções de código

O padrão de cada assunto, para não criar variações novas. Código antigo que ainda não segue um item é corrigido quando alguém mexer nele.

- **Validação do pedido de emissão:** só em `Emissor::validar` (`nfse-emissor`). A emissão e as prévias do app (Emitir, planilha, lote) usam essa função; não copie a conferência para o app.
- **Regras fiscais:** ficam no backend (`nfse-core`, `nfse-emissor`). A interface recebe o resultado e só exibe; listas de códigos e cálculos não são repetidos em JS.
- **Tipos entre crates:** os do `nfse-core` (`Ambiente`, `DpsId`, `Valor`, datas `NaiveDate`) em todas as fronteiras, não `&str` nem um enum paralelo.
- **XML:** um gerador e um escape só, em `nfse-xml`; nada de XML montado com `format!`.
- **Utilitários:** no crate mais baixo que precisa deles, importados pelos outros (`nfse_core::texto` para texto, por exemplo); não copie a função.
- **Erros:** `thiserror` nos crates do núcleo e `String` só na fronteira do IPC e no servidor. Erro de arquivo não vai em `ErroBanco::Regra`. Erro descartado (`let _ =`, `.ok()`) só com comentário dizendo por quê.
- **Enums em texto:** `como_texto()` e `FromStr`.
- **Datas gravadas no banco:** sempre `agora()` (RFC 3339 em UTC com `Z`), para a comparação como texto valer.
- **Comando que usa o cofre:** `emissor!` quando precisa do emissor; `exigir_cofre` + `estado.banco()` quando só lê ou grava o banco. Todo comando que toca dados usa um dos dois.
- **Chamadas da interface:** sempre `chamar` (com o tratamento de erro dele); `invoke` direto só onde o erro é ignorado de propósito, com comentário.
- **Funções longas:** acima de ~140 linhas (contagem do clippy), divida por etapa ao mexer.

## Armadilhas

- `crates/graft/` é cache de ferramenta, sem `Cargo.toml`; está excluída do workspace.
- O banco é cifrado (SQLCipher): copie sempre `emissor.db` e `emissor.cofre` juntos; sem o `.cofre` ou com outra senha ele não abre. Em `Cargo.toml` do `nfse-db`, a build-dependency do `libsqlite3-sys` (que só vale com o `build.rs` do crate) é necessária no Windows ([ADR 0013](docs/decisoes/0013-banco-cifrado-sqlcipher.md)); não remova.
- Ao copiar um banco do Windows para outro sistema, a pasta de saída gravada continua com caminho do Windows; trocar em Configurações.
- A Produção Restrita só aceita contribuintes cadastrados nela (E0084); os testes reais foram em Produção, sempre executados pelo usuário (ver a regra **Produção**).
- `.claude/settings.json` libera os comandos de rotina (cargo fmt/clippy/test/check, git status/diff/log/show/add/commit) e bloqueia `git push --force`, apagar tag e a leitura de `.pfx`, `emissor.db`, `.cofre` e `tmp-não-commitar/`. Não afrouxe esses bloqueios.
