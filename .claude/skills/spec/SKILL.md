---
name: spec
description: Entrevista o usuário e escreve a especificação (spec.md) de uma feature ou mudança de comportamento em docs/specs/NNNN-slug/. Use quando o pedido for uma feature nova, mudança de comportamento público, ou algo ainda vago demais para planejar.
argument-hint: "[descrição curta da ideia]"
---

# /spec — da ideia à especificação

Resultado: `docs/specs/NNNN-<slug>/spec.md` aprovado pelo usuário. **Não escreva código nesta etapa.**

Ideia: $ARGUMENTS

1. **Contexto primeiro.** Leia `docs/arquitetura.md`, as decisões relevantes em `docs/decisoes/`, `docs/fiscal.md` se tocar regra fiscal, e o código que a ideia toca. Tudo que o código responde, descubra sozinho; pergunte só o que ele não responde.
2. **Entrevista.** Use AskUserQuestion em rodadas de até 4 perguntas de múltipla escolha. Cubra: problema e quem sofre com ele; comportamento esperado; o que está fora de escopo; casos de borda e erros; restrições (performance, compatibilidade, segurança, dados, privacidade); como saberemos que está pronto. Quando tiver opinião, coloque a opção recomendada primeiro. Pare quando conseguir escrever critérios de aceite testáveis — não antes, nem muito depois.
3. **Numeração.** `NNNN` = maior número em `docs/specs/` + 1 (4 dígitos, começando em 0001). `slug` em português, kebab-case, sem acentos.
4. **Escreva** a partir de [docs/specs/_template/spec.md](../../../docs/specs/_template/spec.md):
   - requisitos com IDs estáveis (R1, R2…), cada um verificável;
   - critérios de aceite em Dado/Quando/Então, cada um mapeável para um teste;
   - invariantes: o que **não** pode mudar;
   - questões em aberto ficam na seção própria — não invente respostas.
5. **Aprovação.** Mostre um resumo (requisitos + critérios + fora de escopo), não o arquivo inteiro. Ajuste até aprovar e então troque `Status: rascunho` → `aprovada`.
6. **Próximo passo.** Sugira: "Abra uma sessão limpa (`/clear`) e rode `/plan NNNN`" — implementar com o contexto da entrevista atrapalha mais do que ajuda.
