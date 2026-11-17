# 0002 — Busca de município por nome

- **Status:** aprovada <!-- rascunho | aprovada | implementada | abandonada -->
- **Autor:** Felipe CN
- **Data:** 2026-11-17
- **ADRs relacionados:** [0001](../../decisoes/0001-interface-sem-bundler.md) (interface sem bundler)

## Contexto e problema
Três campos do app pedem o município pelo código IBGE de 7 dígitos: o local da prestação, no Emitir nota; o município do tomador, no cadastro de tomador; e o município emissor, no cadastro de empresa. Quase ninguém sabe esse código de cabeça. Hoje há duas saídas:

- digitar o CEP, que preenche o código pela consulta de CEP;
- usar o botão "Não sei o CEP", que busca no ViaCEP por UF, cidade **e rua**.

O caso mais comum no local da prestação é diferente: o tomador é de um município, o prestador de outro, o serviço foi feito num terceiro, e o operador só sabe a cidade. Sem a rua, a busca de CEP não ajuda, e o operador fica sem como informar o local.

## Objetivos
- Informar o município pelo nome nos três campos, sem saber o código IBGE nem o CEP.
- Ver o nome e a UF do município escolhido, não só o código.
- Funcionar sem internet.

## Fora de escopo
- Planilha e e-mail de pedidos aceitarem o nome do município no lugar do código. Pode virar outra spec.
- Mudar a busca "Não sei o CEP" ou a consulta de CEP.
- Prestação no exterior (`cPaisPrestacao`).
- Município padrão de prestação no cadastro do serviço ou na recorrência.
- Qualquer mudança na DPS, no XML ou nas regras de ISS: a nota continua recebendo só o código IBGE.

## Requisitos
| ID | Requisito | Prioridade |
|---|---|---|
| R1 | O `nfse-core` deve embutir a tabela de municípios do IBGE (código de 7 dígitos, nome e UF), gerada a partir do arquivo oficial (DTB), com a fonte na primeira linha, como a lista de serviços e a NBS. | obrigatório |
| R2 | Deve haver uma busca por termo que encontre municípios pelo nome, sem diferença de acento e de maiúscula, pelo nome seguido da UF (ex.: "ribeirao preto sp") e pelo código IBGE, com limite de resultados. | obrigatório |
| R3 | Os campos `#local-prestacao`, `#tomador-municipio` e `#empresa-municipio` devem oferecer a busca: o operador digita parte do nome ou do código, vê uma lista "Nome · UF" com o código, escolhe uma opção, e o campo fica com o código IBGE. | obrigatório |
| R4 | Abaixo de cada um dos três campos deve aparecer o nome e a UF do município cujo código está no campo, venha ele da busca, do CEP, do botão "Usar o endereço do tomador", de refazer/copiar/substituir nota ou do cadastro aberto para edição. | obrigatório |
| R5 | Um código de 7 dígitos que não está na tabela gera só um aviso junto do campo ("não está na tabela de municípios embutida; confira"), sem impedir salvar nem emitir. | obrigatório |
| R6 | A validação de formato existente (7 dígitos) continua valendo; o valor enviado ao backend continua sendo o código IBGE. | obrigatório |
| R7 | A lista de municípios homônimos deve deixar a UF visível para distingui-los (ex.: "Bom Jesus · PI" e "Bom Jesus · RS"). | obrigatório |

## Critérios de aceite
Cada critério vira pelo menos um teste.

- **CA1 (R1)** — Dado a tabela embutida, quando procurada pelo código 3550308, então devolve São Paulo · SP; e a primeira linha do TSV indica o arquivo de origem.
- **CA2 (R1)** — Dado a tabela embutida, então todos os códigos têm 7 dígitos, não se repetem e o total de municípios corresponde ao do arquivo de origem.
- **CA3 (R2)** — Dado o termo "ribeirao preto", quando buscado, então o primeiro resultado é Ribeirão Preto · SP (3543402).
- **CA4 (R2)** — Dado o termo "SÃO JOSÉ DOS CAMPOS" ou "sao jose dos campos", quando buscado, então São José dos Campos · SP aparece nos resultados.
- **CA5 (R2)** — Dado o termo "3543402", quando buscado, então devolve Ribeirão Preto · SP.
- **CA6 (R2, R7)** — Dado o termo "bom jesus", quando buscado, então volta mais de um município, cada um com a sua UF; e o termo "bom jesus rs" põe o do RS primeiro.
- **CA7 (R2)** — Dado um termo sem correspondência ou vazio, quando buscado, então a busca devolve lista vazia, sem erro.
- **CA8 (R3)** — Dado o Emitir nota, quando o operador digita "ribeirao" no município da prestação e escolhe Ribeirão Preto · SP, então o campo fica com 3543402, aparece "Ribeirão Preto/SP" abaixo e a DPS da revisão sai com `cLocPrestacao` 3543402.
- **CA9 (R3)** — Dado o cadastro de tomador e o de empresa, quando o operador busca e escolhe um município pelo nome, então o código é gravado no cadastro como hoje.
- **CA10 (R4)** — Dado uma nota refeita, copiada ou substituída com local da prestação diferente do município da empresa, quando a tela Emitir abre, então o nome e a UF do local aparecem abaixo do campo.
- **CA11 (R5)** — Dado o código 9999999 digitado num dos três campos, quando o campo perde o foco, então aparece o aviso de código fora da tabela, e salvar/emitir não é bloqueado pela tela.
- **CA12 (R6)** — Dado um valor com menos de 7 dígitos, quando o formulário é salvo ou revisado, então o erro de formato atual continua aparecendo junto do campo.

## Casos de borda e erros
- Entrada vazia: no local da prestação, vazio continua valendo como "município da empresa"; sem aviso.
- Termo curto (1 ou 2 letras): a busca pode devolver só os primeiros resultados até o limite, sem travar a tela.
- Código digitado sem escolher da lista: vale como hoje; o nome aparece abaixo se o código estiver na tabela, o aviso se não estiver.
- Município criado depois da tabela embutida: cai no aviso (R5) e a emissão segue; a Sefin é quem decide.
- Falha de dependência externa: não há, a tabela é local.

## Invariantes (o que não pode mudar)
- A DPS e o XML continuam iguais: `cLocPrestacao`, `cMun` do tomador e `cLocEmi` recebem o mesmo código IBGE de hoje.
- O local da prestação continua indo `null` quando vazio ou igual ao município da empresa.
- A consulta de CEP, a busca "Não sei o CEP" e o botão "Usar o endereço do tomador" continuam funcionando como hoje.
- A validação do pedido continua só em `Emissor::validar`; a lista de municípios não é repetida em JS (a busca vem do backend).
- Os cadastros existentes não mudam: nada de migração nem coluna nova.

## Restrições
- Sem internet: a tabela vai embutida no binário (cerca de 5.570 linhas, na ordem de 150 KB).
- Busca rápida o bastante para responder enquanto o operador digita, como a do código de serviço.
- Interface sem bundler e sem estilo em linha; reaproveitar a caixa de busca existente (`ligarCombobox`).

## Questões em aberto
- [ ] Qual versão da DTB do IBGE embutir (a mais recente publicada na data da implementação).

## Histórico de mudanças
| Data | Mudança | Motivo |
|---|---|---|
| 2026-11-17 | Criação | Operador não consegue informar o local da prestação sabendo só a cidade |
