---
title: Pacotes do Devbox precisam vir do mesmo commit do nixpkgs
author: Felipe
date: 2026-10-30
branch: main
cost: { minutes: 14, tokens: 44091 }
id: l-d3f38f3d
scope: repo
status: active
src: agent
touches:
  - ".github/workflows/ci.yml"
  - AGENTS.md
  - README.md
  - compose.yaml
  - devbox.json
  - devbox.lock
  - docs/desenvolvimento.md
  - docs/nuvem.md
  - rust-toolchain.toml
  - servidor/Dockerfile
---

## Decided
gcc e libclang ficam em versões resolvidas no mesmo commit do nixpkgs dos demais pacotes (por isso gcc 16.2.0, e não 15.x).
O job do Windows e o instalador ficam fora do Devbox: o Nix não roda no Windows, e AppImage e DEB ficariam presos às bibliotecas do Nix.
DATABASE_URL só é definida na linha do teste do servidor, nunca no ambiente do Devbox, para o sqlx não lê-la ao compilar nfse-db.

## Tried and ruled out
gcc 15.2.0 e 15.3.0 resolvem para commits mais antigos, com outra glibc: o build script do libsqlite3-sys não consegue abrir o libclang (`undefined symbol: __pointer_chk_guard`).

## Watch out
Sem libclang no Devbox, o bindgen pega o do sistema e falha. O output `lib` do libclang precisa estar declarado, e LIBCLANG_PATH aponta para o perfil.
Depois de `devbox add`, confira no devbox.lock se o pacote novo caiu no mesmo commit dos outros.

In the code: devbox.json, devbox.lock, compose.yaml.
