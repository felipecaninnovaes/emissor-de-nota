# 0001 — Interface em HTML/CSS/JS puro, sem Node nem bundler

Estado: aceita (16/10/2026).

**Contexto.** O sistema é mantido por uma pessoa e distribuído como MSI. Uma cadeia de build JavaScript aumentaria a manutenção.

**Decisão.** A interface do Tauri 2 é HTML, CSS e JS escritos à mão em `crates/app/ui/` (`frontendDist = "ui"`), com as fontes IBM Plex embutidas (a CSP continua `default-src 'self'`).

**Consequências.** Não há dependência de Node; o `app.js` cresce sem módulos de build. Os argumentos de primeiro nível do `invoke` vão em camelCase (`empresaId`, `nfseId`, `dpsId`).
