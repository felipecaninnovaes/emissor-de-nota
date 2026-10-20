# 0016 — Licença AGPL-3.0 com licença comercial alternativa

Estado: aceita (20/10/2026).

**Contexto.** O repositório era MIT. O projeto é um produto de nicho (emissor de NFS-e) com risco real de apropriação: uma softhouse poderia empacotar o núcleo fiscal em produto fechado, ou alguém poderia hospedá-lo como serviço sem devolver nada. O autor quer manter o repositório público e viver de serviços, binário oficial e módulos pagos.

**Decisão.** A partir desta mudança o código novo é `AGPL-3.0-only` (`LICENSE`, `license` em `Cargo.toml`, `copyright` em `tauri.conf.json`). Quem não quiser cumprir a AGPL pode pedir uma licença comercial ao titular dos direitos. Contribuições externas exigem a cessão de direitos descrita em [CONTRIBUTING.md](../../CONTRIBUTING.md), para o licenciamento duplo continuar possível.

**Por quê.**
- A AGPL fecha a brecha do uso em rede, que a GPL comum não cobre.
- Continua sendo open source pela definição da OSI (ao contrário de BUSL ou PolyForm).
- Uso interno, mesmo com modificações, não obriga a publicar nada: escritórios contábeis não são afetados.

**Consequências.**
- As versões até a 1.6.0 já foram distribuídas sob MIT e continuam MIT para quem as obteve.
- Todo o histórico até aqui tem um único autor (três variações de nome, um só e-mail), então o titular pode relicenciar sem consentimento de terceiros. Isso muda com a primeira contribuição externa aceita sem cessão.
- As dependências precisam ser compatíveis com AGPL-3.0 (Apache-2.0, MIT e BSD são). A conferência com `cargo deny check licenses` ainda não foi feita; fazer antes da próxima release, em especial OpenSSL e SQLCipher no instalador.
- Quem integrar o núcleo (`nfse-xml`, `nfse-nacional` etc.) em programa fechado precisa da licença comercial. Se isso virar problema, reavaliar publicar só esses crates sob LGPL ou MIT.
- O instalador deve oferecer o código-fonte correspondente (link para o repositório e a tag) e a licença deve aparecer na tela "Sobre" do app; a AGPL exige que o usuário de versão modificada consiga obter o código.
- A mudança de licença não altera o aviso de responsabilidade do README; a AGPL também exclui garantias (seções 15 e 16).
- Parecer jurídico sobre a titularidade e o texto do CLA ainda é recomendável.
