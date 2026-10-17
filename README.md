# Emissor de NFS-e

Emissor desktop de NFS-e Padrão Nacional (API Sefin Nacional / ADN) para escritórios de contabilidade que emitem em nome de várias empresas, cada uma com seu certificado A1. Feito em Rust com Tauri 2.

- Emite, cancela e consulta notas; gera o DANFSe em PDF e envia XML e PDF por e-mail ao tomador.
- Notas recorrentes, lote do mês e importação de planilha para convênios.
- Certificados cifrados no banco local (SQLite) e protegidos por uma senha mestra.
- Atualiza-se sozinho a partir das Releases do GitHub.

Uso interno de um escritório; o código é público e usa licença MIT.

## Instalação (Windows)

Baixe o `Emissor-NFS-e_<versão>_x64_pt-BR.msi` em [Releases](https://github.com/felipecaninnovaes/emissor-de-nota/releases) e execute. Na primeira abertura, crie a senha mestra (ela não pode ser recuperada). O uso está em [docs/usuario.md](docs/usuario.md).

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

## Licença

MIT. Ver [LICENSE](LICENSE).
