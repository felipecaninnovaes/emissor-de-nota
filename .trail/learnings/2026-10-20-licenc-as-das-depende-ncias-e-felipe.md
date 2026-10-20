---
title: Licenças das dependências e código-fonte no app (AGPL)
author: Felipe
date: 2026-10-20
branch: main
cost: { minutes: 1, tokens: 46860 }
touches:
  - crates/app/ui/app.js
  - crates/app/ui/index.html
  - docs/usuario.md
---

## Decided
- Licenças conferidas com `cargo deny list` (não há deny.toml; `cargo deny check licenses` sem config recusa tudo). Todas compatíveis com AGPL-3.0: MIT, Apache-2.0 (inclui openssl-src 300.x / OpenSSL 3), BSD, ISC, Zlib, MPL-2.0, Unicode-3.0, CDLA-Permissive-2.0, BSL, CC0, Unlicense, 0BSD.
- Oferta do código-fonte (AGPL): cartão "Código-fonte" em Configurações (crates/app/ui/index.html), preenchido em app.js com `…/tree/v<versao>` a partir de `versao_app`.

## Tried and ruled out
- Link clicável `<a href>` para o GitHub: o app não tem plugin opener/shell; o link navegaria a própria webview. URL fica como texto selecionável em `<code>`.

## Watch out
- r-efi aparece como LGPL-2.1-or-later, mas é tri-licenciado (MIT/Apache) e só entra em alvos UEFI.
- SQLCipher (via libsqlite3-sys) é estilo BSD: o aviso de copyright deve acompanhar o instalador.
