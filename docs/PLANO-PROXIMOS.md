# Próximos itens — variáveis na descrição, tomador que retém e copiar nota

Criado em 17/10/2026. Ordem proposta: item 3, depois 1, depois 2 (do mais simples ao que mexe no banco), um commit por item.

## 1. Variáveis na descrição

Hoje só existe `{mes_ref}`, trocado em `aplicar_mes_ref` (`crates/nfse-emissor/src/lib.rs`).

- Trocar por uma função que recebe os dados da nota inteira, com as variáveis:
  - `{mes_ref}` (09/2026), `{mes_ref_extenso}` (setembro de 2026), `{ano_ref}`
  - `{competencia}`, `{data_emissao}`
  - `{tomador}`, `{empresa}`, `{valor}`
- Tornar visíveis: em cada campo de descrição (cadastro de serviço, emissão, recorrência, lote), uma fileira de etiquetas clicáveis que inserem a variável no cursor.
- A revisão mostra o texto já trocado.
- Variável desconhecida (ex.: `{tomadr}`) é barrada antes do envio, para não sair na nota.

## 2. Tomador que retém, sem duplicar o serviço

Hoje o usuário cria dois serviços (com e sem retenção). As alíquotas dependem do serviço (IR 1,5% ou 1%...), mas quem decide se retém é o tomador.

- **Serviço**: as retenções ganham "aplicar: sempre / só para tomador que retém". Os existentes ficam em "sempre" (nada muda no que funciona). Fica no JSON da tributação, sem mudar a tabela.
- **Tomador** (vínculo com a empresa, `empresa_tomador`): "Retém impostos federais" e "Retém ISS". Precisa da migração `0006`.
- **Emissão**: ao escolher o tomador, aparece "Reter nesta nota" já marcado conforme o cadastro; a revisão mostra as retenções e o valor líquido.
- Depois o usuário junta os dois serviços duplicados em um.

## 3. Copiar nota emitida

"Corrigir e emitir" (`refazerNota` em `app.js`, `dados_para_refazer` no emissor) já faz quase tudo, mas só aceita nota rejeitada.

- Nova ação "Copiar" para notas autorizadas e canceladas: leva tomador, serviço, valor, informações complementares e obra para a tela de emissão.
- A descrição é refeita a partir do modelo do serviço com o mês atual (copiar o texto antigo repetiria o mês velho).
- Sai com o próximo número da série.
