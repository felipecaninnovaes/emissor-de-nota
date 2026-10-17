# Configuração

Variáveis de ambiente, opções guardadas pelo aplicativo e locais dos arquivos, com os valores padrão. Para quem desenvolve ou dá suporte. O sistema não usa arquivo `.env`.

## Variáveis de ambiente

| Variável | Onde é lida | Padrão | Uso |
| --- | --- | --- | --- |
| `NFSE_PFX_SENHA` | `nfse-poc` (`crates/nfse-poc/src/main.rs`) | pede no terminal | Senha do `.pfx` nos subcomandos que usam certificado |
| `NFSE_PFX_DIR` | teste `pfx_reais_abrem_ate_o_mac` em `crates/nfse-cert/src/lib.rs` (`#[ignore]`) | — | Pasta com arquivos `.pfx` reais, para conferir que abrem até o MAC |
| `USERPROFILE` | `crates/app/src/comandos/comum.rs` | — | Base da pasta padrão de notas no Windows |
| `HOME` | idem | — | Base da pasta padrão quando não há `USERPROFILE` (Linux, macOS) |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | build do MSI e `.github/workflows/instalador.yml` | — | Assinatura do MSI para o atualizador ([deploy.md](deploy.md)) |

## Opções guardadas no banco

Tabela `configuracao` (chave → valor), lida e gravada por `Banco::configuracao` e `definir_configuracao`.

| Chave | Padrão | Conteúdo |
| --- | --- | --- |
| `pasta_saida` | `<USERPROFILE ou HOME>/Documents/NFS-e` | Pasta onde XML e PDF são gravados; alterada em Configurações |
| `interface_zoom` | `1` | Fator do tamanho do texto: `0.9`, `1`, `1.1`, `1.25`, `1.4` ou `1.5` |
| `interface_densidade` | `auto` | `auto`, `confortavel` ou `compacta` (a automática fica compacta em janelas com menos de 700 px de altura) |
| `smtp` | — | JSON com servidor, porta, segurança, usuário, remetente, nome do remetente, assunto, corpo e envio automático depois da emissão |
| `smtp_senha` | — | Senha do SMTP, cifrada com a chave do cofre (hexadecimal) |
| `cofre` | criado na primeira abertura | Sal, parâmetros do Argon2 e verificador da senha mestra |

## Arquivos e pastas

| O quê | Onde |
| --- | --- |
| Banco | `app_data_dir()/emissor.db`, com identificador `br.emissor-nfse.app`. No Windows, `%APPDATA%\br.emissor-nfse.app\`; no Linux, `~/.local/share/br.emissor-nfse.app/` |
| Tamanho e posição da janela | `app_config_dir()`, arquivo do plugin `window-state`. Na primeira abertura a janela ocupa 90% da área útil do monitor |
| XML e PDF das notas | `<pasta_saida>/<EMPRESA - CNPJ>/<AAAA-MM>/` |
| Certificados reais para a CLI | `certs/` (ignorada pelo git) |

## Opções do build (`crates/app/tauri.conf.json`)

- `identifier`: `br.emissor-nfse.app`; janela mínima 960 × 560, inicial 1280 × 820.
- `bundle.targets`: `["msi"]`, WiX em `pt-BR`; instalação do WebView2 por `downloadBootstrapper`.
- `plugins.updater`: chave pública (`pubkey`), endpoint `https://github.com/felipecaninnovaes/emissor-de-nota/releases/latest/download/latest.json` e `installMode: passive`.
- `tauri.release.conf.json`: acrescenta `createUpdaterArtifacts` (gera a assinatura); usado só no build de release.
- `security.csp`: `default-src 'self'` (sem rede a partir da interface; as consultas externas saem do lado Rust).

## Endereços externos

| Serviço | Endereço | Código |
| --- | --- | --- |
| Sefin Nacional (Produção) | `https://sefin.nfse.gov.br/SefinNacional` | `nfse-nacional/src/lib.rs` |
| Sefin Nacional (Produção Restrita) | `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` | idem |
| Consulta de CNPJ | `https://brasilapi.com.br/api/cnpj/v1/` | `app/src/cnpj.rs` |
| Consulta de CEP | `https://viacep.com.br/ws/` | `app/src/cep.rs` |
| Atualizações | GitHub Releases (endpoint acima) | `tauri.conf.json` |
