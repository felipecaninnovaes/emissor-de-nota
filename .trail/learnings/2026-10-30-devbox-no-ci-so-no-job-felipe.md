---
title: "Devbox no CI só no job Linux, pois o job principal roda no Windows"
author: Felipe
date: 2026-10-30
standing: true
id: l-8a53fdfc
scope: repo
status: active
src: init-sessions
---

## Decided
- O job principal do CI roda no Windows, onde o Devbox não se aplica. Por isso a equipe escolheu usá-lo só no job Linux.
- In the code: Não tente migrar os jobs Windows/macOS do CI para Devbox. Só o job Linux usa `devbox run`.

_From past Claude sessions here, Oct 30._
