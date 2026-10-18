---
title: Assinatura do evento consultado na Sefin não confere (não é bug nosso)
author: Felipe
date: 2026-10-18
branch: correcoes-revisao
cost: { minutes: 32, tokens: 160899 }
touches:
  - docs/fiscal.md
  - crates/app/src/comandos/notas.rs
  - crates/app/src/main.rs
  - crates/app/ui/app.js
  - crates/app/ui/index.html
  - crates/nfse-emissor/src/lib.rs
  - crates/nfse-emissor/src/testes.rs
  - crates/nfse-nacional/src/lib.rs
  - crates/nfse-poc/src/main.rs
  - crates/nfse-xml/src/evento.rs
---

## Decided
- A assinatura da Sefin no XML de evento devolvido por GET /nfse/{chave}/eventos/101101/1 (campo arquivoXml) não é conferida; a sincronização confia no mTLS + e_cancelamento_da_nfse. Documentado em docs/fiscal.md (Assinatura XMLDSIG).
## Tried and ruled out
- Bug no nosso C14N: descartado. Nosso exc-c14n do infEvento é byte a byte igual ao `xmllint --exc-c14n`; o verificador confere as duas assinaturas de uma NFS-e real de Produção Restrita (exc-c14n#WithComments, RSA-SHA256).
- Variantes (manter xmlns repetido no pedRegEvento, sem xmlns, SHA-1, documento inteiro): nenhuma reproduz o DigestValue.
## Watch out
- O pedRegEvento devolvido vem sem a Signature do autor do pedido; hipótese mais provável: a Sefin assinou com ela dentro e a removeu depois. Não dá para confirmar sem um evento com a assinatura original.
- No shell do usuário `cp` e `rm` são interativos: use `command cp`/`command rm -f` em scripts, senão o comando trava ou não sobrescreve.
