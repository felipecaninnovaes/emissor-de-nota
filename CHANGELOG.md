# Changelog

Histórico das versões do aplicativo (`emissor-nfse`), reconstruído a partir das tags e dos commits. Para quem acompanha o que mudou entre instalações. Todas as versões até aqui saíram em 17/10/2026; a versão vem de `crates/app/Cargo.toml`.

## 0.2.5

- Consulta de CNPJ passa a usar a BrasilAPI.
- Tomador que retém: a retenção do serviço só vale quando o tomador retém (migração `0006`).
- Variáveis na descrição: `{mes_ref_extenso}`, `{ano_ref}`, `{competencia}`, `{data_emissao}`, `{tomador}`, `{empresa}`, `{valor}` e `{valor_liquido}`.
- Confirmações da interface em diálogo próprio (o `confirm()` do webview não funciona).

## 0.2.4

- **Copiar** nota autorizada ou cancelada para a tela de emissão.
- DANFSe com a logomarca oficial da NFS-e; procura as fontes Arial do macOS.
- Abrir arquivos e pastas com `open` no macOS.
- `ler_planilha` passa a exigir o cofre aberto.
- `comandos.rs` dividido em módulos (`crates/app/src/comandos/`), sem mudança de comportamento.

## 0.2.3

- Botão **Bloquear**: descarta a chave do cofre e volta à tela de senha.
- O workspace ignora `crates/graft` (cache de ferramenta).
- A tag `v0.2.3` falhou na conferência de versão do workflow por ter sido criada antes de subir a versão.

## 0.2.2

- Segurança: comandos que acessam dados exigem o cofre aberto; `abrir_arquivo` só abre pdf, xml e csv dentro da pasta de saída.
- Argon2, assinatura XMLDSIG e geração do PDF rodam fora das threads assíncronas (`spawn_blocking`).
- Índice `(empresa_id, id DESC)` em `dps` para a listagem de notas (migração `0005`).
- Abrir arquivos e pastas com `xdg-open` fora do Windows; `HOME` como base da pasta padrão.

## 0.2.1

- Importar serviços de planilha ou de outra empresa; exportar o catálogo.
- IBS/CBS no cadastro de serviço e na planilha.
- Grupo de obra na DPS (regra E0370) e busca de CEP no endereço da obra e do tomador.
- PIS/COFINS retidos somados em `vRetCSLL` (NT 007/2026).
- Migrações do banco sempre em CRLF.
- A versão do aplicativo nos XML vem do app (`emissor-nfse/<versão>`).

## 0.2.0

Primeira versão distribuída por instalador MSI, com atualização automática pelo GitHub Releases. Reúne tudo o que foi feito nas Fases 0 e 1 (16 a 17/10/2026):

- Certificado A1 (inclusive `.pfx` em BER), mTLS e assinatura XMLDSIG aceitos pela Sefin; primeira NFS-e real autorizada em Produção.
- Banco SQLite com cofre de certificados (Argon2id + AES-256-GCM), cadastros e numeração.
- Emissão confiável com reconciliação, cancelamento, "Corrigir e emitir" e "Consultar na Sefin".
- Conferência da assinatura da Sefin e arquivamento em `<empresa>/<AAAA-MM>/`.
- DANFSe v2.0 (NT 008/2026) em PDF.
- Interface desktop Tauri 2 com tema escuro, janela e densidade adaptáveis, tabelas, formulários e acessibilidade revisados.
- Notas recorrentes e lote do mês; importação de planilha para convênios; envio por e-mail ao tomador.
