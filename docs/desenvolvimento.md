# Desenvolvimento

Como preparar o ambiente, rodar, testar e alterar o código com segurança. Para quem vai mexer no projeto. A estrutura do código está em [arquitetura.md](arquitetura.md); variáveis e pastas, em [configuracao.md](configuracao.md); publicação de versões, em [deploy.md](deploy.md).

## Ambiente com Devbox (Linux e macOS)

O `devbox.json` fixa as versões do `rustup`, do compilador C, do `libclang`, do Perl, do `xmllint`, do `xmlsec1`, do `cargo-deny`, do `tauri-cli` e, no Linux, do WebKitGTK e do GTK; o `rust-toolchain.toml` fixa o Rust. Com Nix, Devbox, direnv e Docker instalados:

```bash
direnv allow
devbox run setup
```

| Script | O que faz |
| --- | --- |
| `devbox run setup` | Instala a toolchain e baixa as dependências |
| `devbox run run` | Abre o app |
| `devbox run up` / `down` | Sobe e derruba o Postgres de desenvolvimento (`compose.yaml`, porta local 55432) |
| `devbox run test` | Testes do workspace (sem `identidade_mtls_a_partir_do_pfx`, ver [problemas conhecidos](#problemas-conhecidos)), os de XSD e assinatura de `nfse-xml` e os de `servidor/`, que precisam do `devbox run up` |
| `devbox run lint` | `cargo fmt --check`, clippy e `cargo deny check licenses`, no workspace e em `servidor/` |
| `devbox run fmt` | `cargo fmt` no workspace e em `servidor/` |

A toolchain fica em `.devbox/virtenv/rustup`. O `compose.yaml` da raiz é só de desenvolvimento; o de produção é `servidor/compose.yaml` ([nuvem.md](nuvem.md)). Variáveis locais vão em `.env` (ignorado pelo git; modelo em `.env.example`), que o direnv carrega. Os pacotes do Linux e do macOS **de distribuição** não são gerados no Devbox: o executável ficaria preso às bibliotecas do Nix, então o `instalador.yml` compila fora dele. Ao trocar a versão do Rust, atualize `rust-toolchain.toml`, `dtolnay/rust-toolchain@` nos workflows e a imagem em `servidor/Dockerfile`. Ao trocar a versão de um pacote, confira no `devbox.lock` se todos continuam no mesmo commit do nixpkgs: `gcc` e `libclang` de commits diferentes têm glibc diferentes, e o build do `libsqlite3-sys` falha com `undefined symbol: __pointer_chk_guard`.

## Pré-requisitos sem Devbox

- Rust na versão de `rust-toolchain.toml` (o rustup instala sozinho). A versão mínima suportada é a 1.88 (`rust-version` do workspace; edição 2024).
- Windows: o WebView2 (o instalador baixa o bootstrapper). O alvo de distribuição é só Windows (MSI).
- Linux e macOS: o app compila no Linux (testado), mas não há CI para eles ([problemas conhecidos](#problemas-conhecidos)). No Linux o Tauri precisa dos pacotes de desenvolvimento do WebKitGTK 4.1 e do GTK 3 (`pkg-config --modversion webkit2gtk-4.1 gtk+-3.0` deve responder).
- `libclang` (o `bindgen` do `libsqlite3-sys` gera as ligações do SQLCipher).
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

O CI (`.github/workflows/ci.yml`) roda esses três no Windows e, no Ubuntu dentro do Devbox, `cargo test -p nfse-xml -- --include-ignored` (validação contra o XSD oficial com `xmllint` e assinatura contra `xmlsec1`).

Antes de uma release, ou ao adicionar dependência, confira as licenças com `cargo deny check licenses` (precisa do `cargo install cargo-deny`). O `deny.toml` da raiz só permite licenças compatíveis com a AGPL-3.0 ([ADR 0016](decisoes/0016-licenca-agpl.md)); uma dependência com outra licença faz o comando falhar. O CI roda a mesma checagem no job `licencas`.

O `target/` de desenvolvimento já chegou a 30 GB. O `Cargo.toml` raiz reduz o debug info (`debug = "line-tables-only"` no perfil `dev`); `cargo clean` periodicamente ajuda.

## Testes

Os testes ficam junto do código (`#[cfg(test)]`); a Sefin é substituída por um simulado através dos traits `Conector` e `Sefin` (`crates/nfse-emissor/src/testes.rs`). Os certificados de teste são autoassinados, em `crates/nfse-cert/tests/fixtures/` (senha `1234`). Seis testes são `#[ignore]`: três em `nfse-xml` (exigem `xmllint` ou `xmlsec1` no `PATH`; rodam no CI com `-- --include-ignored`), dois em `crates/app` (`cnpj.rs` e `cep.rs`, consultam a rede) e um em `nfse-cert` que confere `.pfx` reais sem saber a senha:

```bash
NFSE_PFX_DIR=<pasta com .pfx> cargo test -p nfse-cert -- --ignored pfx_reais
```

## Migrações do banco

As migrações (`crates/nfse-db/migrations/`) precisam ficar em CRLF: o sqlx guarda no banco o checksum de cada uma, byte a byte, e os bancos em uso foram migrados pelo instalador compilado no Windows. O `.gitattributes` fixa `eol=crlf` nesses arquivos.

Se aparecer `migration N was previously applied but has been modified` ao usar um banco copiado, o arquivo local está com LF: apague `crates/nfse-db/migrations/*.sql` e rode `git checkout -- crates/nfse-db/migrations` para o `.gitattributes` valer. Se persistir, `touch` nos `.sql` ou `cargo clean -p nfse-db` (o `sqlx::migrate!` embute os arquivos na compilação).

Nova migração: o número seguinte ao do último arquivo de `crates/nfse-db/migrations/`, arquivo em CRLF. Mudança na tabela `dps` não vai em migração: o ajuste `dps_unica_por_ambiente` recria a tabela depois das migrações num banco novo; use `crates/nfse-db/src/ajustes.rs` ([arquitetura.md](arquitetura.md#banco-de-dados)).

## Comandos Tauri

Cada `#[tauri::command]` fica em `crates/app/src/comandos/<assunto>.rs` e precisa estar na lista de `generate_handler!` do `crates/app/src/main.rs`, com o caminho completo (`comandos::empresas::listar_empresas`). Um `pub use` que reexporta a função **não funciona**: o macro gera itens ocultos no módulo onde a função está.

Esquecer um comando no `generate_handler!` compila, mas a tela quebra ao chamá-lo. Ao mexer em comandos, compare a lista de nomes antes e depois:

```bash
grep -rhA1 'tauri::command' crates/app/src/comandos | grep 'fn ' | sort
```

Todo comando que lê ou grava dados precisa de `exigir_cofre(&estado)` ou da macro `emissor!` (`comandos/comum.rs`).

## Interface

`crates/app/ui/` é HTML, CSS e JS sem bundler. O JS fica em módulos ES nativos: `app.js` é a entrada e importa `js/<tela ou assunto>.js`; código de uma tela vai no módulo dela, e o que mais de uma tela usa vai em `util.js`, `tabela.js`, `formularios.js` ou `componentes.js` (`util`, `formularios` e `componentes` não importam telas). Variável `let` exportada só é reatribuída pelo próprio módulo (import é somente leitura); para mudá-la de fora, exporte uma função. Argumentos de primeiro nível do `invoke` vão em camelCase (`empresaId`, `nfseId`, `dpsId`). Critérios usados para validar mudanças visuais: sem rolagem horizontal da página de 960×560 a 2560×1440 (pixels lógicos), fluxos completos só com teclado, todo controle com nome acessível, erro de validação junto do campo, modo de alto contraste do Windows. O histórico da melhoria de usabilidade está em [_arquivo/plano-usabilidade.md](_arquivo/plano-usabilidade.md).

A logomarca do DANFSe é `crates/app/assets/logo-nfse.png`, gerada a partir de `logo-nfse.svg` (cópia ajustada: fonte Liberation Sans e largura 240).

## Lista de Serviços Nacional e NBS

A API do Sistema Nacional não lista os códigos de tributação nacional (`cTribNac`) nem a NBS (`cNBS`); as duas tabelas vêm do Anexo B da [documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual) (`anexo_b-nbs2-lista_servico_nacional-snnfse-*.xlsx`, abas `LISTA.SERV.NAC.` e `LISTA.NBS_v2.0`). Elas ficam embutidas em `crates/nfse-core/dados/lista-servicos-nacional.tsv` (módulo `nfse_core::lista_servicos`) e `crates/nfse-core/dados/nbs.tsv` (módulo `nfse_core::nbs`), com a leitura e a busca comuns em `nfse_core::tabela_codigos`. Os campos de código do cadastro de serviços buscam nelas (comandos `buscar_lista_servicos` e `buscar_nbs`). A primeira linha de cada TSV guarda o nome do arquivo de origem, que a tela mostra como versão quando um código não está na tabela. Da NBS entram só os códigos de 9 dígitos; o grupo de cada um é a descrição do nível imediatamente acima (vazio na posição sem subdivisões, `X.XXXX.00.00`). O anexo não traz correlação entre `cTribNac` e NBS.

Quando sair uma versão nova do anexo, baixe a planilha **sem renomear** e regenere:

```bash
cargo run -p nfse-core --example gerar_lista_servicos -- caminho/anexo_b-nbs2-lista_servico_nacional-snnfse-vX-XX-AAAAMMDD.xlsx
cargo test -p nfse-core -- lista_servicos nbs
```

O gerador grava os dois TSV, confere o código de cada linha da lista de serviços com item, subitem e desdobro e falha se a planilha mudar de formato (na NBS, código com mais de 9 dígitos); as linhas de modelo sem descrição no fim da aba da NBS são ignoradas. As tabelas não bloqueiam o cadastro: código fora delas só gera um aviso.

## Classificação tributária e indicador da operação do IBS/CBS (cClassTrib e cIndOp)

O `cClassTrib` (6 dígitos) e o CST (3 dígitos) do grupo `IBSCBS` são informados no cadastro do serviço; a DPS só leva os dois códigos e a Sefin calcula alíquotas, reduções e valores (por exemplo, `200029`, serviços de saúde humana, vem com CST `200` e redução de 60% no IBS e na CBS). A tabela oficial fica embutida em `crates/nfse-core/dados/class-trib.tsv` (módulo `nfse_core::class_trib`) e vem da página [Classificação Tributária](https://dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria) do Portal DFe (SVRS), que não oferece planilha: a tabela está embutida no HTML da página, em JSON. Cada `cClassTrib` pertence a um único CST, então o par é conferido em `Dps::validar`, em `Servico::validar` (cadastro e importação) e, na tela, escolher o código preenche o CST. Código fora da tabela não bloqueia (a tabela pode atrasar); par CST/`cClassTrib` errado bloqueia. O campo do cadastro busca por código ou nome (comando `buscar_class_trib`) e mostra o CST e a redução do código escolhido. Na planilha de serviços, CST em branco com `cClassTrib` conhecido assume o CST da tabela.

O `cIndOp` (6 dígitos, o local da operação) vem da tabela [Indicadores dos Locais de Operação](https://dfe-portal.svrs.rs.gov.br/CFF/TabelaIndicadoresDosLocaisDeOperacao) do mesmo portal, que é uma tabela HTML comum. Fica em `crates/nfse-core/dados/ind-operacao.tsv` (módulo `nfse_core::ind_operacao`; a descrição é o nome do indicador e o grupo é o local da operação) e o campo do cadastro busca nela (comando `buscar_ind_operacao`). O `cIndOp` não é derivado de outro campo e a tabela não o relaciona ao `cTribNac`: escolher o indicador certo do serviço continua sendo do operador (por exemplo, consulta médica presencial é `030101`, e não o `100301` de "demais serviços"). Código fora da tabela só gera aviso.

Para atualizar, baixe as páginas e regenere:

```bash
curl -sL https://dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria -o class-trib.html
cargo run -p nfse-core --example gerar_class_trib -- class-trib.html
curl -sL https://dfe-portal.svrs.rs.gov.br/CFF/TabelaIndicadoresDosLocaisDeOperacao -o ind-operacao.html
cargo run -p nfse-core --example gerar_ind_operacao -- ind-operacao.html
cargo test -p nfse-core -- class_trib ind_operacao
```

A primeira linha de cada TSV guarda a origem e a data da geração. Os geradores falham se o portal mudar de formato, se um código não tiver 6 dígitos, se o CST de um código diferir do grupo dele ou se houver código repetido (e, no `cIndOp`, se uma linha tiver número de colunas diferente ou vigência encerrada).

## CLI `nfse-poc`

Diagnóstico direto na Sefin, sem o app. Subcomandos: `cert-info`, `gerar-dps`, `assinar`, `danfse`, `verificar`, `cancelar`, `consultar-dps`, `consultar-eventos`, `emitir`. A senha do certificado vem da variável `NFSE_PFX_SENHA` ou é pedida no terminal.

```bash
cargo run -p nfse-poc -- cert-info --pfx caminho/empresa.pfx
```

```bash
cargo run -p nfse-poc -- consultar-dps --pfx caminho/empresa.pfx --id DPS351110221122233300018100001000000000000001
```

Se o mTLS funcionar, a consulta responde que não há NFS-e para a DPS (404) ou devolve a chave. `consultar-eventos --pfx … --chave <chave> --ambiente restrita` só lê: mostra as respostas brutas de três rotas de eventos e se o cancelamento foi encontrado. Para gerar, assinar e emitir em Produção Restrita, parta de [exemplos/dps-exemplo.json](../exemplos/dps-exemplo.json), guarde a cópia em `certs/` (ignorada pelo git) e use:

```bash
cargo run -p nfse-poc -- gerar-dps --entrada certs/dps.json --saida certs/dps-assinada.xml --agora --pfx caminho/empresa.pfx
cargo run -p nfse-poc -- emitir --pfx caminho/empresa.pfx --xml certs/dps-assinada.xml --ambiente restrita --saida certs/nfse.xml
```

Emitir ou cancelar em Produção exige `--ambiente producao --confirmo-producao`. Lembre que a Produção Restrita só aceita contribuintes cadastrados nela (E0084; ver [fiscal.md](fiscal.md#ambientes)).

```bash
cargo run -p nfse-poc -- cancelar --pfx caminho/empresa.pfx --chave <chave de 50 posições> --motivo erro-na-emissao --justificativa "Valor informado errado" --ambiente producao --confirmo-producao
```

## Log e diagnóstico

O app usa `tracing`: `crates/app/src/registro.rs` grava `<dados>/logs/emissor.AAAA-MM-DD.log` (rotação diária, 14 arquivos, horário UTC) e registra pânicos. Os crates `nfse-emissor` e `nfse-nacional` emitem os eventos (`info!` no ciclo da emissão, `warn!` nas rejeições e falhas de rede, `debug!` no envio e na resposta); o filtro só abre o nível dos crates do emissor, e as bibliotecas ficam em aviso. O modo detalhado (`debug`) é um arquivo-marca `log-detalhado` na pasta de dados, porque o log sobe antes de o banco abrir. **Nunca** logar senha, chave privada, XML completo ou dados pessoais: use ids (`dps`, `chave`) e códigos.

A coluna `dps.resposta_sefin` guarda o corpo da última resposta de erro da Sefin (até 20 mil caracteres, limpa a cada novo envio); o XML enviado já fica em `dps.xml_assinado`. `Emissor::diagnostico_nota` junta tudo num texto com os dados pessoais mascarados (`mascarar_xml` em `crates/nfse-emissor/src/diagnostico.rs`; ao incluir um elemento novo com dado pessoal na DPS, acrescente-o em `ELEMENTOS_MASCARADOS`). Os códigos de rejeição conhecidos e a orientação ao operador ficam em `crates/nfse-nacional/src/mensagens.rs`.

## Segurança do repositório

O repositório é público. **Nunca** versionar certificados reais, senhas, bancos, XMLs ou PDFs de clientes. O `.gitignore` bloqueia `*.pfx`, `*.p12`, `*.pem`, `*.key`, `*.db`, `/certs/`, `.env`, entre outros; as únicas exceções são os certificados autoassinados de teste em `tests/fixtures/`.

## Problemas conhecidos

- **`cargo test --workspace` falha no Linux** em `nfse-nacional::testes::identidade_mtls_a_partir_do_pfx` (`Identidade("builder error")`). O teste monta a identidade mTLS com `native-tls` a partir de `teste-ecnpj-legacy.pfx`, que usa RC2-40; o OpenSSL 3 não abre esse algoritmo sem o provedor `legacy` (`openssl pkcs12 -in teste-ecnpj-legacy.pfx -passin pass:1234 -noout` falha; com `-legacy` passa). No Windows o SChannel aceita. Com `--no-fail-fast`, nos demais crates passam 135 testes e 6 ficam ignorados (execução de 17/10/2026, OpenSSL 3.6).
- **Sem CI para macOS.** Os nomes das fontes Arial do macOS, a abertura de pastas com `open` e os caminhos não foram confirmados em um Mac.
- **Banco copiado do Windows** guarda a pasta de saída com caminho do Windows. No Linux e no macOS, troque em Configurações antes de emitir; senão o app cria um diretório com esse nome dentro do diretório atual.
- **Linux: seletores de pasta e de arquivo pelo xdg-desktop-portal.** O `tauri-plugin-dialog` usa a feature `xdg-portal` (sem `gtk3`), então "Escolher pasta…" e "Escolher arquivo…" abrem o diálogo do ambiente (KDE, GNOME). Sem um backend de portal instalado (`xdg-desktop-portal-kde`, `-gnome` ou `-gtk`) o diálogo não abre. Por isso a interface não usa `<input type="file">`, que o WebKitGTK abre no diálogo do GTK: os campos de arquivo chamam os comandos de `comandos/dialogo.rs` (`ler_arquivo_escolhido` devolve nome e bytes só do arquivo escolhido no diálogo).
- **Linux/KDE com Wayland: o app roda pelo XWayland** para o KWin desenhar a barra de título ([configuracao.md](configuracao.md#variáveis-de-ambiente), `GDK_BACKEND`). Com escala fracionária a janela pode ficar menos nítida; `GDK_BACKEND=wayland` desfaz.
- **Pasta `crates/graft/`** é cache de ferramenta, sem `Cargo.toml`; o workspace a exclui (`exclude = ["crates/graft"]`).
