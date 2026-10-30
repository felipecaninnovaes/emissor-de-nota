---
title: Sempre conferir o remoto antes de amend em commit que pode já ter sido enviado
author: Felipe
date: 2026-10-30
standing: true
id: l-ab035e0e
scope: repo
status: active
src: init-sessions
---

## Watch out
- Um amend em commit já enviado pelo usuário deixou a main local divergente do remoto. Foi preciso refazer a branch com reset suave.
- In the code: Antes de `git commit --amend`, rode `git fetch` e `git status -sb`. Se o commit já está em origin, faça um commit novo. Nunca force push.

_From past Claude sessions here, Oct 30._
