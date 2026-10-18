# Publicação e operação

Como gerar o instalador, publicar uma versão e como o aplicativo se atualiza. Para quem publica versões. Configurações do build estão em [configuracao.md](configuracao.md).

## Instaladores

O build local gera o pacote da plataforma em que roda. Windows: MSI. macOS: `.app` e DMG (`--bundles app,dmg`). Linux: AppImage e DEB (`--bundles appimage,deb`; precisam de `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf` e `libxdo-dev`).

Local (o Tauri baixa o WiX na primeira vez; o arquivo sai em `target/release/bundle/msi/`):

```bash
cargo install tauri-cli --version "^2" --locked
```

```bash
cd crates/app && cargo tauri build
```

Os pacotes saem em `target/release/bundle/` (com `--target`, em `target/<alvo>/release/bundle/`). O build local não assina. Para assinar, defina `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` e acrescente `--config tauri.release.conf.json`.

## Publicar uma versão

1. Suba `version` em `crates/app/Cargo.toml` e atualize o `Cargo.lock` (um `cargo build` ou `cargo check` faz isso).
2. Commit.
3. Crie e envie a tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

O workflow **Instalador** (`.github/workflows/instalador.yml`) roda ao receber uma tag `v*` ou manualmente (Actions → Instalador → Run workflow; nesse caso os pacotes ficam só como artefatos). Ele tem dois jobs:

1. **build**, em matriz (Windows, macOS Apple Silicon, macOS Intel, Linux Ubuntu 22.04): confere que a tag é igual à versão de `crates/app/Cargo.toml` e falha se não for (aconteceu com a `v0.2.3`); no Windows roda `cargo test --workspace`; gera os pacotes assinados (`cargo tauri build --config tauri.release.conf.json --bundles ...`); renomeia sem espaços (o GitHub troca espaço por ponto nos anexos) e guarda um fragmento com a assinatura e o endereço de cada plataforma;
2. **publicar** (só em tag, depois de todos os builds): junta os fragmentos no `latest.json` e cria a Release com os pacotes, os `.sig` e o `latest.json`. Se uma plataforma falhar, nada é publicado.

| Plataforma (`latest.json`) | Baixado pelo atualizador | Instalação manual |
|---|---|---|
| `windows-x86_64` | `Emissor-NFS-e_<v>_x64_pt-BR.msi` | o mesmo MSI |
| `darwin-aarch64` | `Emissor-NFS-e_<v>_aarch64.app.tar.gz` | `..._aarch64.dmg` |
| `darwin-x86_64` | `Emissor-NFS-e_<v>_x64.app.tar.gz` | `..._x64.dmg` |
| `linux-x86_64` | `Emissor-NFS-e_<v>_amd64.AppImage` | o AppImage ou `..._amd64.deb` |

## Atualização automática

O app consulta `releases/latest/download/latest.json` ao abrir e em Configurações → Atualizações (`crates/app/src/comandos/atualizacao.rs`). Se há versão nova, baixa o MSI, confere a assinatura com a chave pública de `plugins.updater.pubkey` (em `tauri.conf.json`), fecha, instala (`installMode: passive`) e abre de novo. O `latest.json` traz as quatro plataformas da tabela acima. No Linux só a instalação por AppImage se atualiza sozinha; quem instalou o DEB baixa a versão nova à mão.

## Chave de assinatura

- A chave privada fica em `~/.tauri/emissor-nfse.key` (gerada com `cargo tauri signer generate`); o conteúdo e a senha vão para os segredos do repositório `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- Sem os segredos, o MSI não é assinado e o build da Release falha.
- **Perder a chave** obriga a reinstalar manualmente a versão que traga a chave pública nova: as máquinas existentes não aceitam atualização assinada por outra chave.
- Pendente: assinatura de código do MSI (alerta do SmartScreen) e assinatura/notarização da Apple para o macOS (sem elas, o Gatekeeper pede "Abrir mesmo assim" na primeira execução). A assinatura do Tauri, que protege a atualização, é outra coisa e já funciona ([escopo.md](escopo.md)).

## Operação

- **Dados**: um `emissor.db` por máquina; o backup cifrado é configurado em Configurações (pasta fora da máquina). Para copiar o banco para outra máquina, copie `emissor.db` **e** `emissor.cofre` (juntos; o banco é cifrado e só abre com a senha mestra) e confira as [migrações em CRLF](desenvolvimento.md#migrações-do-banco) e a pasta de saída em Configurações.
- **Senha mestra**: não há recuperação. Sem ela, os certificados guardados no banco não podem ser decifrados; é preciso reimportar os `.pfx`.
- **Notas incertas** (Sefin sem resposta) são reconciliadas ao abrir o sistema ou pelo botão "Verificar".
