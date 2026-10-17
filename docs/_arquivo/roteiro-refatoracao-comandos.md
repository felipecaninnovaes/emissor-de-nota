# Roteiro: dividir `crates/app/src/comandos.rs` em módulos

> **Arquivado em 17/10/2026.** Plano concluído; fica como histórico. O estado atual está em [docs/](../).

Ponto de partida: 1.437 linhas e 52 comandos `#[tauri::command]` em um arquivo só. O arquivo só é
referenciado pela lista `generate_handler!` do `main.rs`. O único teste do crate está em `cnpj.rs`.

## Regras

- Só mover código. Nenhuma mudança de comportamento, de nome de comando ou de assinatura.
- Um módulo por commit, para poder reverter um a um.
- Trabalhar na branch `refatora-comandos`, sem funcionalidade nova em paralelo.
- Erros tipados (`Result<T, String>` → enum) ficam **fora** desta refatoração: mudam o contrato com a
  interface, que hoje faz `String(e)`.

## Passo 0: linha de base

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo fmt --all --check` verdes.
- Guardar a lista dos 52 nomes de comando. Depois de cada passo, comparar com a lista do
  `generate_handler!` do `main.rs`: nenhum comando pode sumir nem duplicar.

## Passo 1: transformar em diretório

`git mv crates/app/src/comandos.rs crates/app/src/comandos/mod.rs`. Compila igual e preserva o histórico.

## Passo 2: extrair o que é compartilhado (`comandos/comum.rs`)

`R`, `texto`, `exigir_cofre`, `vazio_para_none`, `mes_de`, `pasta_padrao`, `pasta_configurada`,
`abrir_no_sistema` e a macro `emissor!`. A macro depende da ordem de declaração dos módulos: declarar
`comum` primeiro, com `#[macro_use]` ou `pub(crate) use emissor;`.

## Passo 3: um módulo por vez, das pontas para o centro

| Ordem | Módulo | Conteúdo |
|---|---|---|
| 1 | `atualizacao.rs` | atualização do aplicativo |
| 2 | `email.rs` | configuração e envio de e-mail |
| 3 | `aparencia.rs` | zoom e densidade |
| 4 | `cofre.rs` | cofre, pasta de saída, `iniciar` |
| 5 | `tomadores.rs` | tomadores |
| 6 | `servicos.rs` | catálogo de serviços e planilha de serviços |
| 7 | `empresas.rs` | empresas e certificados |
| 8 | `recorrencias.rs` | recorrências e lote do mês |
| 9 | `planilha.rs` | importação de planilha (convênios) |
| 10 | `emissao.rs` e `notas.rs` | emissão, notas, cancelamento (o mais acoplado: fica por último) |

Em cada módulo: mover DTOs, impls e comandos juntos, ajustar os `use`, atualizar o caminho no
`generate_handler!`, rodar `cargo check`, `clippy` e `fmt`, e commitar.

### Armadilha do Tauri

`#[tauri::command]` gera itens ocultos (`__cmd__nome`) no módulo onde a função está. O
`generate_handler!` precisa do caminho completo (`comandos::empresas::listar_empresas`). Um `pub use`
que reexporta a função em `comandos::` **não funciona**.

## Passo 4: depois de dividir

Testes unitários das funções puras (`validar_pedido`, `vazio_para_none`, `mes_de`, `pasta_padrao`).
Meta: arquivos de 100 a 300 linhas.

## Verificação final

- CI verde no Windows.
- MSI manual instalado e um fluxo completo: abrir o app, emitir em homologação, abrir PDF e pasta,
  bloquear e desbloquear, importar planilha, enviar e-mail.

## Risco principal

Esquecer de registrar um comando movido no `generate_handler!`: o app compila, mas a tela quebra ao
chamar o comando. A comparação da lista de nomes do passo 0 cobre isso.
