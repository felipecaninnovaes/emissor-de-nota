# 0017 — Backup na nuvem com servidor próprio

Estado: aceita (27/10/2026).

**Contexto.** O backup `.nfbk` já sai cifrado com a chave da senha mestra ([arquitetura.md](../arquitetura.md#backup-cifrado)), mas fica numa pasta que o escritório precisa levar para fora da máquina. O autor quer oferecer o backup na nuvem como serviço pago, com o código aberto (inclusive o servidor), hospedado numa VPS própria que já tem backup diário.

**Decisão.**
- Servidor em `servidor/` (pacote `emissor-nuvem`, Rust/axum, `[workspace]` próprio, fora do build do aplicativo), com três containers no `servidor/compose.yaml`: o painel, o Postgres do painel e um S3 (`versitygw` com backend POSIX). A imagem do painel é publicada em `ghcr.io/felipecaninnovaes/emissor-nuvem` pelo workflow `nuvem.yml`. Instalação e operação: [nuvem.md](../nuvem.md).
- **O aplicativo só fala com o painel** (`/api/v1`, `Authorization: Bearer <token>`); o painel repassa o arquivo em streaming para o S3, que fica na rede interna do compose. Um endereço público só, nenhuma credencial do S3 no aplicativo, e o bloqueio feito no painel vale na hora. Sem URL pré-assinada.
- **Desativar o cliente bloqueia só o envio.** Listar e baixar continuam liberados, para a restauração não depender da assinatura; apagar os backups é ação manual do administrador.
- **Token por cliente**: `nfb_` + 32 bytes aleatórios, mostrado uma vez; no banco só o SHA-256. Gerar outro invalida o anterior.
- **Retenção no servidor**: os `MANTER_BACKUPS` mais recentes por cliente (padrão 30) e no máximo `TAMANHO_MAXIMO_MB` por arquivo (padrão 500). Cada envio confere o SHA-256 informado pelo aplicativo.
- **Painel** com um único administrador (variáveis de ambiente), sessão em cookie `HttpOnly; Secure; SameSite=Strict` gravada no Postgres, conferência do `Origin` nos formulários e bloqueio depois de 5 senhas erradas em 15 minutos. HTML gerado no servidor, sem JavaScript.

**Por quê.**
- O servidor nunca consegue abrir o backup: a cifragem já acontece no aplicativo, com a senha mestra, que não sai da máquina.
- `versitygw` grava cada objeto como arquivo comum, então o backup diário da VPS leva arquivos legíveis. O MinIO foi descartado (repositório arquivado em abril de 2026, sem imagens novas) e o Garage exige configurar layout e guarda os dados em formato próprio.
- Servidor aberto não ameaça o serviço: o que se vende é a hospedagem e a manutenção; quem quiser hospedar o próprio pode.

**Consequências.**
- Os testes do servidor que usam Postgres rodam só no CI (job `servidor`, com `--include-ignored`), como os do `xmllint`.
- A imagem é só `linux/amd64`; arm64 exigiria compilar em emulação ou cruzado.
- Sem redundância própria: a durabilidade depende do backup da VPS, que precisa levar a pasta `servidor/dados`. A cópia do Postgres em arquivo só é consistente com snapshot atômico do disco.
- Tráfego de envio e download passa pelo painel; para o volume esperado (um arquivo por escritório por dia), não pesa.
