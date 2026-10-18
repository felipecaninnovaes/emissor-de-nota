---
title: Integração com o KDE no Linux (portal e barra de título)
author: Felipe
date: 2026-10-18
branch: linux-integracao-kde
cost: { minutes: 2, tokens: 17056 }
touches:
  - Cargo.lock
  - crates/app/Cargo.toml
  - crates/app/src/main.rs
  - docs/configuracao.md
  - docs/desenvolvimento.md
---

## Decided
- tauri-plugin-dialog com `default-features = false, features = ["xdg-portal"]`: no Linux o seletor vai pelo xdg-desktop-portal (diálogo do KDE/GNOME). `gtk3` e `xdg-portal` do rfd não convivem.
- No KDE com Wayland, `preferir_x11_no_kde` (main.rs) define `GDK_BACKEND=x11` se há `DISPLAY` e nenhum `GDK_BACKEND`: o KWin desenha a barra Breeze.
## Tried and ruled out
- Migrar para Slint só pelo visual no KDE: custo de 4 a 8 semanas (interface, atualizador, instaladores); o Slint resolveria a barra nativa no Wayland, mas o seletor seria o mesmo rfd+portal.
- Tema breeze-gtk: o GTK3 no Wayland sempre desenha CSD (sem xdg-decoration), então só imita a Breeze.
## Watch out
- XWayland pode deixar a janela borrada com escala fracionária; `GDK_BACKEND=wayland` desfaz.
- `std::env::set_var` é `unsafe` na edição 2024; chamar antes de qualquer thread.
