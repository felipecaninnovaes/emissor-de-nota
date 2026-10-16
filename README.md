# Emissor de NFS-e Multiempresa

Emissor desktop de NFS-e Padrão Nacional (API Sefin Nacional / ADN), em Rust, para escritório
de contabilidade que emite em nome de várias empresas clientes, cada uma com seu certificado A1.

Escopo completo: [Escopo — Emissor de NFS-e Multiempresa (Rust).md](<Escopo — Emissor de NFS-e Multiempresa (Rust).md>).

## Situação

**Fase 0 — prova de conceito** (critério de continuidade do projeto):

| Item | Situação |
| --- | --- |
| Leitura de .pfx A1 (formato legado 3DES/RC2 e AES) | feito, testado |
| Diagnóstico do certificado (CNPJ ICP-Brasil, validade, AC, EKU Client Auth) | feito, testado |
| C14N inclusivo + XMLDSIG RSA-SHA1 sem prefixo `ds:` | feito; conferido com lxml e (no CI) com xmlsec1 |
| Cliente mTLS da Sefin (emitir, consultar DPS, consultar NFS-e) | feito; **falta testar com certificado real em Produção Restrita** |
| Montagem da DPS completa e validação XSD | próxima etapa |

## Estrutura

| Crate | Papel |
| --- | --- |
| `nfse-core` | Domínio sem I/O: CNPJ (inclusive alfanumérico), CPF, Id da DPS |
| `nfse-cert` | Certificado A1: leitura do .pfx, diagnóstico, assinatura RSA |
| `nfse-xml` | Canonicalização e assinatura/verificação XMLDSIG |
| `nfse-nacional` | Cliente mTLS da Sefin Nacional, GZip+Base64, reconciliação |
| `nfse-poc` | CLI da prova de conceito |

## Testando com um certificado real

A senha do certificado é lida da variável `NFSE_PFX_SENHA` ou pedida no terminal.

```bash
cargo run -p nfse-poc -- cert-info --pfx caminho/empresa.pfx
```

```bash
cargo run -p nfse-poc -- consultar-dps --pfx caminho/empresa.pfx --id DPS351110221122233300018100001000000000000001
```

Se a conexão mTLS funcionar, a consulta responde que não há NFS-e para a DPS (ou devolve a chave, se houver).
Depois, para assinar e emitir uma DPS em Produção Restrita (homologação, sem valor fiscal):

```bash
cargo run -p nfse-poc -- assinar --pfx caminho/empresa.pfx --xml dps.xml --saida dps-assinada.xml
```

```bash
cargo run -p nfse-poc -- emitir --pfx caminho/empresa.pfx --xml dps-assinada.xml --ambiente restrita --saida nfse.xml
```

Emitir em Produção exige `--ambiente producao --confirmo-producao`.

## Segurança

Este repositório é público. **Nunca** versionar certificados reais, senhas, bancos de dados ou XMLs
de clientes; o `.gitignore` bloqueia `*.pfx`, `*.p12`, `*.db`, `.env` e afins. Os únicos certificados
versionados são os autoassinados de teste em `crates/nfse-cert/tests/fixtures/` (senha `1234`, sem valor).

## Desenvolvimento

```bash
cargo test --workspace
```

## Licença

MIT — ver [LICENSE](LICENSE).
