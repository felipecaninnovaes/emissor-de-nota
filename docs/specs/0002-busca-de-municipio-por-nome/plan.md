# 0002 — Plano técnico

- **Spec:** [spec.md](spec.md)
- **Status:** aprovado

## Abordagem
A tabela de municípios vai embutida no `nfse-core`, como a Lista de Serviços Nacional e a NBS: `crates/nfse-core/dados/municipios.tsv` com `# fonte: RELATORIO_DTB_BRASIL_2025_MUNICIPIOS.xls` na primeira linha e depois `código\tnome\tUF`, ordenado por código. O TSV sai de um exemplo novo, `crates/nfse-core/examples/gerar_municipios.rs`, no molde de `gerar_lista_servicos.rs` (calamine já é dev-dependency): acha a linha de cabeçalho pelas colunas "Código Município Completo" e "Nome_Município" (não pela posição), tira a UF dos dois primeiros dígitos do código, confere 7 dígitos e repetição e imprime o total. Fonte: DTB 2025 do IBGE.

O módulo `crates/nfse-core/src/municipios.rs` segue `lista_servicos.rs` (`OnceLock`, `fonte()`, `itens()`, `por_codigo()`, `buscar()`), com o tipo próprio `Municipio { codigo, nome, uf }` e a sua própria `uf_do_ibge`, usada pelo gerador. O `nfse-pdf` continua independente (sem depender do `nfse-core`, como em [arquitetura.md](../../arquitetura.md)) e mantém a cópia dele.

Busca: termo vazio → lista vazia; só dígitos → códigos que começam com eles. Texto: normaliza (`nfse_core::texto::sem_acento_minusculo`, pontuação como espaço, para "Embu-Guaçu" e "Sant'Ana"); casa quem tem todas as palavras no nome; se a última palavra é sigla de UF, casa também quem tem as demais palavras no nome e é dessa UF. Ordem: a UF pedida primeiro; depois nome igual, nome que começa com o termo, nome que contém; desempate pelo nome normalizado. Limite de 30.

No app, o comando `buscar_municipios` (`crates/app/src/comandos/municipios.rs`) devolve `{ fonte, itens }`, sem cofre, como `buscar_lista_servicos` (`crates/app/src/comandos/servicos.rs`). Na interface, `ligarCampoDeCodigo` (`crates/app/ui/js/servicos.js:87`) passa para `componentes.js`, com os parâmetros opcionais `opcao` e `ajudaDe`; `ligarCampoMunicipio` usa esse componente nos três campos, que passam a aceitar texto (formato `ibge` vira `codigoDeLista(7, …)` em `formularios.js`). Todo trecho que põe um código no campo sem digitação (CEP, CNPJ, "Usar o endereço do tomador", refazer/copiar/substituir, abrir cadastro, limpar) chama o `descrever` devolvido, para o nome aparecer abaixo.

## Alternativas descartadas
| Alternativa | Por que não |
|---|---|
| Reusar `tabela_codigos::Tabela` (colunas código/descrição/grupo = código/nome/UF) | Ordena pela posição na tabela e põe "contém" no mesmo nível de "igual": "sao paulo" traria São Paulo das Missões (RS, código menor) antes de São Paulo; e "bom jesus rs" não põe o do RS primeiro. |
| Buscar o município em serviço externo (API do IBGE, ViaCEP) | A spec exige funcionar sem internet. |
| Lista de municípios em JS | A spec proíbe repetir a lista no JS; a busca vem do backend. |
| Mover `uf_do_ibge` do `nfse-pdf` para o `nfse-core` e fazer o PDF importá-la | Criaria a dependência `nfse-pdf` → `nfse-core`, e o PDF é independente por desenho; a cópia de 27 siglas é o custo menor. |

## Mudanças
| Módulo / arquivo | Mudança | Requisitos |
|---|---|---|
| `crates/nfse-core/src/municipios.rs`, `lib.rs` | módulo novo: tabela, `por_codigo`, `buscar`, `uf_do_ibge` | R1, R2, R7 |
| `crates/nfse-core/dados/municipios.tsv` | tabela gerada da DTB 2025 | R1 |
| `crates/nfse-core/examples/gerar_municipios.rs` | gerador do TSV | R1 |
| `crates/app/src/comandos/municipios.rs`, `comandos/mod.rs`, `main.rs` | comando `buscar_municipios` registrado | R2, R3 |
| `crates/app/ui/index.html` | três campos com caixa de busca; `#empresa-municipio-nome` | R3, R4 |
| `crates/app/ui/js/formularios.js` | formato `ibge` aceita texto e exige 7 dígitos | R6 |
| `crates/app/ui/js/componentes.js`, `servicos.js` | `ligarCampoDeCodigo` compartilhado; `ligarCampoMunicipio` | R3, R4, R5, R7 |
| `crates/app/ui/js/consultas.js`, `emitir.js`, `notas.js`, `tomadores.js`, `empresas.js` | chamar `descrever` onde o código é posto sem digitação | R4 |
| `CHANGELOG.md`, `docs/desenvolvimento.md`, `docs/usuario.md` | linha no `## Não publicado`; como regenerar a tabela; uso na tela | — |

## Contratos alterados
- Comando Tauri novo `buscar_municipios(termo) -> { fonte, itens: [{ codigo, nome, uf }] }`.
- `nfse_core::municipios` público.
- Nada muda na DPS, no XML, no banco nem nos comandos de emissão e cadastro: o valor enviado continua o código IBGE.

## Estratégia de teste
| Requisito / CA | Tipo de teste | Onde |
|---|---|---|
| R1 / CA1, CA2 | unitário | `nfse_core::municipios::testes` |
| R2, R7 / CA3–CA7 | unitário | `nfse_core::municipios::testes` |
| R2 | unitário do comando | `crates/app/src/comandos/municipios.rs` |
| R3–R6 / CA8–CA12 | manual no app (`just run`) | roteiro em [tasks.md](tasks.md) |

## Riscos e mitigação
- Binário ~150 KB maior: aceito na spec.
- Caminho que põe o código sem chamar `descrever` deixa a ajuda velha: lista de chamadas na T4 e roteiro do CA10.
- Nome digitado sem escolher da lista: dá erro de formato no campo (antes nem dava para digitar letras), em vez de o local virar vazio e a nota sair no município da empresa.

## Rollback
Reverter os commits. Sem migração nem dado novo gravado.

## Decisões que viram ADR
- Nenhuma: segue o padrão das tabelas embutidas.
