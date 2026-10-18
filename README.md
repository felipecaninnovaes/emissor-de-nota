# Emissor de NFS-e

Emissor desktop de NFS-e Padrão Nacional (API Sefin Nacional / ADN) para escritórios de contabilidade que emitem em nome de várias empresas, cada uma com seu certificado A1. Feito em Rust com Tauri 2.

- Emite, cancela e consulta notas; gera o DANFSe em PDF e envia XML e PDF por e-mail ao tomador.
- Notas recorrentes, lote do mês e importação de planilha para convênios.
- Banco local (SQLite) cifrado em repouso com SQLCipher, protegido por uma senha mestra.
- Atualiza-se sozinho a partir das Releases do GitHub.

Uso interno de um escritório; o código é público e usa licença MIT.

## Instalação

Baixe o instalador da plataforma em [Releases](https://github.com/felipecaninnovaes/emissor-de-nota/releases):

| Sistema | Arquivo |
| --- | --- |
| Windows | `Emissor-NFS-e_<versão>_x64_pt-BR.msi` |
| macOS (Apple Silicon / Intel) | `Emissor-NFS-e_<versão>_aarch64.dmg` / `..._x64.dmg` (ainda sem assinatura da Apple) |
| Linux | `Emissor-NFS-e_<versão>_amd64.AppImage` (atualiza-se sozinho) ou `..._amd64.deb` |

Na primeira abertura, crie a senha mestra (ela não pode ser recuperada). O uso está em [docs/usuario.md](docs/usuario.md).

## Desenvolvimento

Precisa de Rust 1.88 ou mais novo.

```bash
git clone https://github.com/felipecaninnovaes/emissor-de-nota.git
cargo run -p emissor-nfse
```

```bash
cargo test --workspace
```

Setup completo, testes, CLI de diagnóstico e problemas conhecidos: [docs/desenvolvimento.md](docs/desenvolvimento.md).

## Documentação

| Documento | Conteúdo |
| --- | --- |
| [docs/usuario.md](docs/usuario.md) | Guia do operador |
| [docs/arquitetura.md](docs/arquitetura.md) | Crates, fluxo de emissão, banco, cofre |
| [docs/desenvolvimento.md](docs/desenvolvimento.md) | Ambiente, testes, convenções, `nfse-poc` |
| [docs/configuracao.md](docs/configuracao.md) | Variáveis, opções e arquivos |
| [docs/fiscal.md](docs/fiscal.md) | Regras da Sefin Nacional |
| [docs/deploy.md](docs/deploy.md) | Gerar o MSI e publicar versões |
| [docs/escopo.md](docs/escopo.md) | Escopo e roadmap |
| [docs/decisoes/](docs/decisoes/) | Decisões de projeto |
| [CHANGELOG.md](CHANGELOG.md) | Histórico de versões |
| [AGENTS.md](AGENTS.md) | Instruções para agentes de código |

## Segurança

O repositório é público: **nunca** versione certificados reais, senhas, bancos de dados nem XMLs de clientes. O `.gitignore` bloqueia `*.pfx`, `*.p12`, `*.db`, `.env` e afins; os únicos certificados versionados são os autoassinados de teste (senha `1234`) em `crates/nfse-cert/tests/fixtures/`.

## Aviso

Este software não substitui a validação contábil e fiscal. Alíquotas, códigos de serviço, retenções e regime tributário são responsabilidade de quem configura o sistema. Antes de emitir em Produção, confirme com a contabilidade e com a prefeitura se o município aceita o emissor e o padrão utilizado. O sistema avisa de retenções que parecem indevidas, mas não valida se uma retenção é cabível ao serviço ([docs/fiscal.md](docs/fiscal.md)). O software é fornecido "como está", sem garantia de qualquer tipo (ver [LICENSE](LICENSE)).

### Segurança dos dados

O banco é cifrado e os certificados ficam protegidos pela senha mestra, mas isso não basta para proteger os dados. **A segurança dos dados é responsabilidade de quem instala e opera o sistema:** a guarda da senha mestra, dos certificados A1 e dos backups (`.nfbk`, `emissor.db`, `emissor.cofre`); o controle de acesso, a atualização e a proteção das máquinas; a pasta onde XML e PDF são gravados, que fica **sem cifra**; e o cumprimento da legislação de proteção de dados (LGPD) sobre as informações de empresas e tomadores.

O desenvolvedor não se responsabiliza por vazamento, perda ou uso indevido de dados, nem por qualquer dano decorrente do uso do software, nos termos da [LICENSE](LICENSE).

## Licença

MIT. Ver [LICENSE](LICENSE).
