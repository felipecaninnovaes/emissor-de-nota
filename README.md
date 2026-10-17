# Emissor de NFS-e Multiempresa

Emissor desktop de NFS-e Padrão Nacional (API Sefin Nacional / ADN), em Rust, para escritório
de contabilidade que emite em nome de várias empresas clientes, cada uma com seu certificado A1.

Escopo completo: [Escopo — Emissor de NFS-e Multiempresa (Rust).md](<Escopo — Emissor de NFS-e Multiempresa (Rust).md>).

## Situação

- **Fase 0 (prova de conceito) e Fase 1 (uso diário no escritório): concluídas em 16/10/2026.** O aplicativo
  emite, cancela e envia por e-mail NFS-e reais em Produção; tem recorrentes com lote do mês e importação de
  planilha para convênios. Produção Restrita não serve de teste para o escritório (rejeição E0084).
- **Fase 2 (instalação e operação): em andamento** — instalador MSI e CI prontos.
- Andamento, decisões e pendências: [docs/ESTADO.md](docs/ESTADO.md).

## Instalação (Windows)

Baixe o `Emissor-NFS-e_<versão>_x64_pt-BR.msi` da página de
[Releases](https://github.com/felipecaninnovaes/emissor-de-nota/releases) e execute. O banco fica em
`%APPDATA%\br.emissor-nfse.app\emissor.db` (um por usuário da máquina) e as notas em `Documentos\NFS-e`
(configurável). Na primeira abertura o sistema pede para criar a senha mestra, que protege os certificados
e não pode ser recuperada.

Depois de instalado, o sistema procura uma versão nova a cada abertura (e em Configurações → Atualizações)
e se atualiza sozinho: baixa o MSI da última Release, confere a assinatura, fecha, instala e abre de novo.

## Estrutura

| Crate | Papel |
| --- | --- |
| `nfse-core` | Domínio sem I/O: CNPJ (inclusive alfanumérico), CPF, Id da DPS, modelo e validações do leiaute |
| `nfse-cert` | Certificado A1: leitura do .pfx (inclusive BER), diagnóstico, assinatura RSA |
| `nfse-xml` | DPS e eventos, C14N (inclusivo e exclusivo), assinatura e verificação XMLDSIG (RSA-SHA1/SHA-256) |
| `nfse-nacional` | Cliente mTLS da Sefin Nacional, GZip+Base64, orientações por código de rejeição |
| `nfse-db` | SQLite: cofre (Argon2id + AES-256-GCM), cadastros, numeração, DPS/NFS-e, recorrências, importações, e-mail |
| `nfse-emissor` | Emissão confiável, reconciliação, cancelamento, arquivamento, planilhas e envio por e-mail |
| `nfse-pdf` | DANFSe v2.0 (NT 008/2026) |
| `nfse-poc` | CLI de diagnóstico (certificado, DPS, emissão e consulta direto na Sefin) |
| `app` | Aplicativo desktop Tauri 2 (`emissor-nfse`), interface em HTML/CSS/JS sem bundler |

## Testando com um certificado real

A senha do certificado é lida da variável `NFSE_PFX_SENHA` ou pedida no terminal.

```bash
cargo run -p nfse-poc -- cert-info --pfx caminho/empresa.pfx
```

```bash
cargo run -p nfse-poc -- consultar-dps --pfx caminho/empresa.pfx --id DPS351110221122233300018100001000000000000001
```

Se a conexão mTLS funcionar, a consulta responde que não há NFS-e para a DPS (ou devolve a chave, se houver).
Depois, para gerar, assinar e emitir uma DPS em Produção Restrita (homologação, sem valor fiscal), parta de
[exemplos/dps-exemplo.json](exemplos/dps-exemplo.json) com os dados da empresa (guarde a cópia em `certs/`, fora do git):

```bash
cargo run -p nfse-poc -- gerar-dps --entrada certs/dps.json --saida certs/dps-assinada.xml --agora --pfx caminho/empresa.pfx
```

```bash
cargo run -p nfse-poc -- emitir --pfx caminho/empresa.pfx --xml certs/dps-assinada.xml --ambiente restrita --saida certs/nfse.xml
```

Emitir em Produção exige `--ambiente producao --confirmo-producao`. Para cancelar (motivo `erro-na-emissao`,
`servico-nao-prestado` ou `outros`; justificativa de 15 a 255 caracteres):

```bash
cargo run -p nfse-poc -- cancelar --pfx caminho/empresa.pfx --chave <chave de 50 posições> --motivo erro-na-emissao --justificativa "Valor informado errado" --ambiente producao --confirmo-producao
```

## Segurança

Este repositório é público. **Nunca** versionar certificados reais, senhas, bancos de dados ou XMLs
de clientes; o `.gitignore` bloqueia `*.pfx`, `*.p12`, `*.db`, `.env` e afins. Os únicos certificados
versionados são os autoassinados de teste em `crates/nfse-cert/tests/fixtures/` (senha `1234`, sem valor).

## Desenvolvimento

```bash
cargo test --workspace
```

```bash
cargo run -p emissor-nfse
```

Gerar o instalador MSI (o Tauri baixa o WiX na primeira vez; o arquivo sai em `target/release/bundle/msi/`):

```bash
cargo install tauri-cli --version "^2" --locked
```

```bash
cd crates/app && cargo tauri build
```

No GitHub, o workflow **Instalador** gera o MSI manualmente (Actions → Instalador → Run workflow) ou ao
publicar uma tag `vX.Y.Z` igual à versão de `crates/app/Cargo.toml`, que vira uma Release com o MSI, a
assinatura (`.sig`) e o `latest.json` lido pelo atualizador.

Publicar uma versão:

1. subir `version` em `crates/app/Cargo.toml` e fazer o commit;
2. `git tag vX.Y.Z` e `git push origin vX.Y.Z`.

O atualizador só aceita MSI assinado com a chave cuja parte pública está em `plugins.updater.pubkey` do
`tauri.conf.json`. A chave privada (`~/.tauri/emissor-nfse.key`, gerada com `cargo tauri signer generate`)
e a senha dela ficam nos segredos `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` do
repositório. Perder a chave obriga a reinstalar manualmente a versão com a chave nova. O `cargo tauri build`
local continua sem assinatura; para assinar, defina as duas variáveis e use `--config tauri.release.conf.json`.

## Licença

MIT — ver [LICENSE](LICENSE).
