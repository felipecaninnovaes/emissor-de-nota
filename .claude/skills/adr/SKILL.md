---
name: adr
description: Registra uma decisão de projeto em docs/decisoes/ no formato das decisões existentes. Use quando uma decisão for difícil de reverter — stack, formato de dados, leiaute fiscal, contrato público, infraestrutura, segurança, dependência estrutural — ou quando um plano marcar "→ ADR".
argument-hint: "[título da decisão]"
---

# /adr — registrar decisão

Decisão: $ARGUMENTS

1. **Número:** maior `docs/decisoes/NNNN-*.md` + 1 (4 dígitos). Slug em português, kebab-case, sem acentos.
2. **Colete:** contexto (forças em jogo), pelo menos 2 opções consideradas, decisão, motivo, consequências positivas e negativas. O que não estiver na conversa, no código ou na spec, pergunte — não invente motivação.
3. **Escreva** `docs/decisoes/NNNN-<slug>.md` no formato das existentes (ex.: [0016](../../../docs/decisoes/0016-licenca-agpl.md)): título `# NNNN — <decisão>`, linha `Estado: proposta.`, depois **Contexto.**, **Decisão.**, **Por quê.** e **Consequências.** Quando o usuário aprovar, troque para `Estado: aceita (DD/MM/AAAA).`
4. **Substituição:** se substitui outra decisão, marque a antiga como `Estado: substituída por NNNN` (é a única edição permitida numa decisão aceita).
5. Se a decisão muda fronteiras do sistema, atualize [docs/arquitetura.md](../../../docs/arquitetura.md) e cite a decisão lá.
