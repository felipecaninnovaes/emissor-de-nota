# Publicação e operação

Como gerar o instalador, publicar uma versão e como o aplicativo se atualiza. Para quem publica versões. Configurações do build estão em [configuracao.md](configuracao.md).

## Instalador MSI

Local (o Tauri baixa o WiX na primeira vez; o arquivo sai em `target/release/bundle/msi/`):

```bash
cargo install tauri-cli --version "^2" --locked
```

```bash
cd crates/app && cargo tauri build
```

O build local não assina. Para assinar, defina `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` e acrescente `--config tauri.release.conf.json`.

## Publicar uma versão

1. Suba `version` em `crates/app/Cargo.toml` e atualize o `Cargo.lock` (um `cargo build` ou `cargo check` faz isso).
2. Commit.
3. Crie e envie a tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

O workflow **Instalador** (`.github/workflows/instalador.yml`) roda ao receber uma tag `v*` ou manualmente (Actions → Instalador → Run workflow; nesse caso o MSI fica só como artefato). Ele:

1. confere que a tag é igual à versão de `crates/app/Cargo.toml` e falha se não for (aconteceu com a `v0.2.3`, criada antes de subir a versão);
2. roda `cargo test --workspace` no Windows;
3. gera o MSI assinado (`cargo tauri build --config tauri.release.conf.json`);
4. renomeia para `Emissor-NFS-e_<versão>_x64_pt-BR.msi` (o GitHub troca espaço por ponto nos anexos) e monta o `latest.json`;
5. cria a Release com o MSI, o `.sig` e o `latest.json`.

## Atualização automática

O app consulta `releases/latest/download/latest.json` ao abrir e em Configurações → Atualizações (`crates/app/src/comandos/atualizacao.rs`). Se há versão nova, baixa o MSI, confere a assinatura com a chave pública de `plugins.updater.pubkey` (em `tauri.conf.json`), fecha, instala (`installMode: passive`) e abre de novo. Só o Windows tem plataforma no `latest.json` (`windows-x86_64`).

## Chave de assinatura

- A chave privada fica em `~/.tauri/emissor-nfse.key` (gerada com `cargo tauri signer generate`); o conteúdo e a senha vão para os segredos do repositório `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- Sem os segredos, o MSI não é assinado e o build da Release falha.
- **Perder a chave** obriga a reinstalar manualmente a versão que traga a chave pública nova: as máquinas existentes não aceitam atualização assinada por outra chave.
- Pendente: assinatura de código do MSI, que evitaria o alerta do SmartScreen ([escopo.md](escopo.md)).

## Operação

- **Dados**: um `emissor.db` por máquina; o backup cifrado é configurado em Configurações (pasta fora da máquina). Para copiar o banco para outra máquina, copie `emissor.db` **e** `emissor.cofre` (juntos; o banco é cifrado e só abre com a senha mestra) e confira as [migrações em CRLF](desenvolvimento.md#migrações-do-banco) e a pasta de saída em Configurações.
- **Senha mestra**: não há recuperação. Sem ela, os certificados guardados no banco não podem ser decifrados; é preciso reimportar os `.pfx`.
- **Notas incertas** (Sefin sem resposta) são reconciliadas ao abrir o sistema ou pelo botão "Verificar".
