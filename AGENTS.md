# AGENTS.md

Instruções para agentes de código (e pessoas) que alteram este repositório. Detalhes ficam em [docs/](docs/); aqui só o que evita erro.

## O projeto

Emissor desktop de NFS-e Padrão Nacional, em Rust: workspace `crates/*` com o núcleo fiscal (`nfse-core`, `nfse-cert`, `nfse-xml`, `nfse-nacional`, `nfse-db`, `nfse-emissor`, `nfse-pdf`), a CLI `nfse-poc` e o app Tauri 2 (`crates/app`, pacote `emissor-nfse`) com interface em HTML/CSS/JS sem bundler. Mapa dos crates e fluxo de emissão: [docs/arquitetura.md](docs/arquitetura.md).

## Comandos

```bash
cargo run -p emissor-nfse
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

O CI roda fmt, clippy e testes no Windows. No Linux, `nfse-nacional::testes::identidade_mtls_a_partir_do_pfx` falha porque o OpenSSL 3 não abre o `.pfx` legado de teste; os outros passam ([docs/desenvolvimento.md](docs/desenvolvimento.md#problemas-conhecidos)).

## Regras

- **O código é a fonte da verdade.** Se um documento divergir, corrija o documento. Ao mudar comportamento, atualize o documento dono do assunto (lista em [README.md](README.md#documentação)).
- **Repositório público:** nunca versionar certificados reais, senhas, bancos, XMLs ou PDFs de clientes. Nunca registrar senha, chave privada ou XML completo em log.
- **Dados de clientes ou do escritório** (nomes, volumes, municípios) não entram na documentação.
- **Migrações** (`crates/nfse-db/migrations/`): arquivos novos em CRLF e a próxima numeração é a `0009`. Nunca editar uma migração já aplicada (o sqlx confere o checksum). Ver [docs/desenvolvimento.md](docs/desenvolvimento.md#migrações-do-banco).
- **Comandos Tauri:** todo `#[tauri::command]` novo vai em `crates/app/src/comandos/<assunto>.rs`, entra no `generate_handler!` de `main.rs` com o caminho completo (`pub use` não funciona) e, se lê ou grava dados, chama `exigir_cofre(&estado)` ou usa a macro `emissor!`. Esquecer o registro compila e quebra a tela.
- **Emissão:** a DPS é validada antes de reservar número; a DPS é gravada como `enviando` antes do envio; em resposta incerta, consultar a Sefin antes de reenviar. Não quebre essas garantias ([docs/fiscal.md](docs/fiscal.md)).
- **Decisões já tomadas** que não devem ser revertidas sem conversa: [docs/decisoes/](docs/decisoes/) (por exemplo, `synchronous = FULL`, assinatura da Sefin só como aviso, interface sem bundler).
- **Interface:** argumentos de primeiro nível do `invoke` em camelCase; sem estilo em linha; erros de validação junto do campo.
- **Versão e release:** a tag `vX.Y.Z` precisa bater com `version` em `crates/app/Cargo.toml` (e o `Cargo.lock`); ver [docs/deploy.md](docs/deploy.md).
- **Idioma:** código, comentários e mensagens em português do Brasil, no estilo do código vizinho.
- Antes de commitar: `cargo fmt --all`, clippy sem avisos e os testes dos crates alterados.

## Armadilhas

- `crates/graft/` é cache de ferramenta, sem `Cargo.toml`; está excluída do workspace.
- O banco é cifrado (SQLCipher): copie sempre `emissor.db` e `emissor.cofre` juntos; sem o `.cofre` ou com outra senha ele não abre. Em `Cargo.toml` do `nfse-db`, a build-dependency do `libsqlite3-sys` (que só vale com o `build.rs` do crate) é necessária no Windows ([ADR 0013](docs/decisoes/0013-banco-cifrado-sqlcipher.md)); não remova.
- Ao copiar um banco do Windows para outro sistema, a pasta de saída gravada continua com caminho do Windows; trocar em Configurações.
- A Produção Restrita só aceita contribuintes cadastrados nela (E0084); os testes reais foram em Produção. Nunca emita em Produção sem intenção.
