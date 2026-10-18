# Desenvolvimento

Como preparar o ambiente, rodar, testar e alterar o código com segurança. Para quem vai mexer no projeto. A estrutura do código está em [arquitetura.md](arquitetura.md); variáveis e pastas, em [configuracao.md](configuracao.md); publicação de versões, em [deploy.md](deploy.md).

## Pré-requisitos

- Rust 1.88 ou mais novo (`rust-version` do workspace; edição 2024).
- Windows: o WebView2 (o instalador baixa o bootstrapper). O alvo de distribuição é só Windows (MSI).
- Linux e macOS: o app compila no Linux (testado), mas não há CI para eles ([problemas conhecidos](#problemas-conhecidos)). No Linux o Tauri precisa dos pacotes de desenvolvimento do WebKitGTK 4.1 e do GTK 3 (`pkg-config --modversion webkit2gtk-4.1 gtk+-3.0` deve responder).
- Compilador C e Perl (o SQLCipher embutido compila o OpenSSL junto; no Windows o Strawberry Perl que já vem nos runners do GitHub basta). A primeira compilação demora alguns minutos a mais.
- Para os testes marcados `#[ignore]`: `xmllint` (pacote `libxml2-utils`) e `xmlsec1`.
- Para gerar o MSI: `cargo install tauri-cli --version "^2" --locked`.

## Comandos do dia a dia

```bash
cargo run -p emissor-nfse
```

```bash
cargo test --workspace
```

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

O CI (`.github/workflows/ci.yml`) roda esses três no Windows e, no Ubuntu, `cargo test -p nfse-xml -- --include-ignored` (validação contra o XSD oficial com `xmllint` e assinatura contra `xmlsec1`).

O `target/` de desenvolvimento já chegou a 30 GB. O `Cargo.toml` raiz reduz o debug info (`debug = "line-tables-only"` no perfil `dev`); `cargo clean` periodicamente ajuda.

## Testes

Os testes ficam junto do código (`#[cfg(test)]`); a Sefin é substituída por um simulado através dos traits `Conector` e `Sefin` (`crates/nfse-emissor/src/testes.rs`). Os certificados de teste são autoassinados, em `crates/nfse-cert/tests/fixtures/` (senha `1234`). Seis testes são `#[ignore]`: três em `nfse-xml` (exigem `xmllint` ou `xmlsec1` no `PATH`; rodam no CI com `-- --include-ignored`), dois em `crates/app` (`cnpj.rs` e `cep.rs`, consultam a rede) e um em `nfse-cert` que confere `.pfx` reais sem saber a senha:

```bash
NFSE_PFX_DIR=<pasta com .pfx> cargo test -p nfse-cert -- --ignored pfx_reais
```

## Migrações do banco

As migrações (`crates/nfse-db/migrations/`) precisam ficar em CRLF: o sqlx guarda no banco o checksum de cada uma, byte a byte, e os bancos em uso foram migrados pelo instalador compilado no Windows. O `.gitattributes` fixa `eol=crlf` nesses arquivos.

Se aparecer `migration N was previously applied but has been modified` ao usar um banco copiado, o arquivo local está com LF: apague `crates/nfse-db/migrations/*.sql` e rode `git checkout -- crates/nfse-db/migrations` para o `.gitattributes` valer. Se persistir, `touch` nos `.sql` ou `cargo clean -p nfse-db` (o `sqlx::migrate!` embute os arquivos na compilação).

Nova migração: próximo número é o `0007`, arquivo em CRLF.

## Comandos Tauri

Cada `#[tauri::command]` fica em `crates/app/src/comandos/<assunto>.rs` e precisa estar na lista de `generate_handler!` do `crates/app/src/main.rs`, com o caminho completo (`comandos::empresas::listar_empresas`). Um `pub use` que reexporta a função **não funciona**: o macro gera itens ocultos no módulo onde a função está.

Esquecer um comando no `generate_handler!` compila, mas a tela quebra ao chamá-lo. Ao mexer em comandos, compare a lista de nomes antes e depois:

```bash
grep -rhA1 'tauri::command' crates/app/src/comandos | grep 'fn ' | sort
```

Todo comando que lê ou grava dados precisa de `exigir_cofre(&estado)` ou da macro `emissor!` (`comandos/comum.rs`).

## Interface

`crates/app/ui/` é HTML, CSS e JS sem bundler. Argumentos de primeiro nível do `invoke` vão em camelCase (`empresaId`, `nfseId`, `dpsId`). Critérios usados para validar mudanças visuais: sem rolagem horizontal da página de 960×560 a 2560×1440 (pixels lógicos), fluxos completos só com teclado, todo controle com nome acessível, erro de validação junto do campo, modo de alto contraste do Windows. O histórico da melhoria de usabilidade está em [_arquivo/plano-usabilidade.md](_arquivo/plano-usabilidade.md).

A logomarca do DANFSe é `crates/app/assets/logo-nfse.png`, gerada a partir de `logo-nfse.svg` (cópia ajustada: fonte Liberation Sans e largura 240).

## CLI `nfse-poc`

Diagnóstico direto na Sefin, sem o app. Subcomandos: `cert-info`, `gerar-dps`, `assinar`, `danfse`, `verificar`, `cancelar`, `consultar-dps`, `emitir`. A senha do certificado vem da variável `NFSE_PFX_SENHA` ou é pedida no terminal.

```bash
cargo run -p nfse-poc -- cert-info --pfx caminho/empresa.pfx
```

```bash
cargo run -p nfse-poc -- consultar-dps --pfx caminho/empresa.pfx --id DPS351110221122233300018100001000000000000001
```

Se o mTLS funcionar, a consulta responde que não há NFS-e para a DPS (404) ou devolve a chave. Para gerar, assinar e emitir em Produção Restrita, parta de [exemplos/dps-exemplo.json](../exemplos/dps-exemplo.json), guarde a cópia em `certs/` (ignorada pelo git) e use:

```bash
cargo run -p nfse-poc -- gerar-dps --entrada certs/dps.json --saida certs/dps-assinada.xml --agora --pfx caminho/empresa.pfx
cargo run -p nfse-poc -- emitir --pfx caminho/empresa.pfx --xml certs/dps-assinada.xml --ambiente restrita --saida certs/nfse.xml
```

Emitir ou cancelar em Produção exige `--ambiente producao --confirmo-producao`. Lembre que a Produção Restrita só aceita contribuintes cadastrados nela (E0084; ver [fiscal.md](fiscal.md#ambientes)).

```bash
cargo run -p nfse-poc -- cancelar --pfx caminho/empresa.pfx --chave <chave de 50 posições> --motivo erro-na-emissao --justificativa "Valor informado errado" --ambiente producao --confirmo-producao
```

## Segurança do repositório

O repositório é público. **Nunca** versionar certificados reais, senhas, bancos, XMLs ou PDFs de clientes. O `.gitignore` bloqueia `*.pfx`, `*.p12`, `*.pem`, `*.key`, `*.db`, `/certs/`, `.env`, entre outros; as únicas exceções são os certificados autoassinados de teste em `tests/fixtures/`.

## Problemas conhecidos

- **`cargo test --workspace` falha no Linux** em `nfse-nacional::testes::identidade_mtls_a_partir_do_pfx` (`Identidade("builder error")`). O teste monta a identidade mTLS com `native-tls` a partir de `teste-ecnpj-legacy.pfx`, que usa RC2-40; o OpenSSL 3 não abre esse algoritmo sem o provedor `legacy` (`openssl pkcs12 -in teste-ecnpj-legacy.pfx -passin pass:1234 -noout` falha; com `-legacy` passa). No Windows o SChannel aceita. Com `--no-fail-fast`, nos demais crates passam 135 testes e 6 ficam ignorados (execução de 17/10/2026, OpenSSL 3.6).
- **Sem CI para macOS.** Os nomes das fontes Arial do macOS, a abertura de pastas com `open` e os caminhos não foram confirmados em um Mac.
- **Banco copiado do Windows** guarda a pasta de saída com caminho do Windows. No Linux e no macOS, troque em Configurações antes de emitir; senão o app cria um diretório com esse nome dentro do diretório atual.
- **Linux: seletor de pasta pelo xdg-desktop-portal.** O `tauri-plugin-dialog` usa a feature `xdg-portal` (sem `gtk3`), então o "Escolher pasta…" abre o diálogo do ambiente (KDE, GNOME). Sem um backend de portal instalado (`xdg-desktop-portal-kde`, `-gnome` ou `-gtk`) o diálogo não abre.
- **Linux/KDE com Wayland: o app roda pelo XWayland** para o KWin desenhar a barra de título ([configuracao.md](configuracao.md#variáveis-de-ambiente), `GDK_BACKEND`). Com escala fracionária a janela pode ficar menos nítida; `GDK_BACKEND=wayland` desfaz.
- **Pasta `crates/graft/`** é cache de ferramenta, sem `Cargo.toml`; o workspace a exclui (`exclude = ["crates/graft"]`).
