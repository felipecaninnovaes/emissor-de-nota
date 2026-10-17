# Estado do projeto e próximos passos

Atualizado em 16/10/2026. Ponto de partida para a próxima sessão de trabalho.

## Onde estamos

**Fase 0 — concluída.** Certificado A1 real, mTLS, assinatura XMLDSIG e leiaute aceitos pela Sefin;
primeira NFS-e autorizada em Produção (e cancelada). Produção Restrita não serve de teste para o
escritório: o contribuinte não está no CNC desse ambiente (rejeição E0084).

**Fase 1 — objetivo: o escritório trocar o Emissor Web por este sistema no dia a dia.**

| Item | Situação |
| --- | --- |
| 1. Banco SQLite + cofre de certificados (`nfse-db`) | feito |
| 2. Emissão confiável, reconciliação, cancelamento (`nfse-emissor`) | feito |
| 3. Conferir assinatura da Sefin e arquivar em `<empresa>/<AAAA-MM>/` | feito |
| 4. DANFSe v2.0 em PDF (`nfse-pdf`, NT 008/2026) | feito |
| 5. Interface desktop Tauri 2 (`crates/app`) | feito (falta instalador MSI e CI) |
| 6a. Notas recorrentes (`{mes_ref}`) e lote do mês | feito |
| 6b. Importação de planilha para convênios (valores variáveis) | feito |
| 7. Envio por e-mail ao tomador | pendente |

## Item 5 — o que falta

- `crates/app/src/comandos.rs`: comandos Tauri prontos e compilando (cofre, pasta, empresas,
  certificado, tomadores, serviços, prévia, emitir, notas, cancelar, reconciliar, pendentes, abrir PDF/pasta).
- `crates/app/ui/index.html` e `estilo.css`: telas e diálogos prontos.
- `crates/app/ui/app.js` escrito e conferido no navegador com os comandos simulados (dados fictícios).
  Argumentos de primeiro nível do `invoke` vão em camelCase (`empresaId`, `nfseId`, `dpsId`).
  Em 16/10/2026 o app emitiu e cancelou uma NFS-e real em Produção. Comportamentos:
  - trava: `situacao_cofre` → criar (senha + confirmação) ou abrir; mostrar `pendencias` da abertura;
  - seletor de empresa no topo (lembrar a última em `localStorage`), selo do ambiente, validade do certificado;
  - emitir: busca de tomador com `buscar_tomadores`, serviço preenche valor e descrição, mês de
    referência padrão = mês anterior, valor digitado como `1.500,00` → enviar `1500,00`;
    "Revisar" chama `previa_dps` (mostra erros; Produção em destaque) e só então `emitir`;
  - notas: filtro por mês, selos por situação, abrir PDF/pasta (`abrir_nota`), cancelar, reconciliar incertas;
  - cadastros: empresa (+ importar `.pfx` lendo o arquivo como bytes), numeração/IM por ambiente,
    tomador (`Tomador` do `nfse-core` em JSON: `documento: {"cnpj": "..."}`), serviço (`Tributacao`
    com `retencoes` em percentuais e `total_tributos` = `"nao_informado"` | `{"percentual_simples": "11.48"}` | `{"percentuais": {...}}`);
  - configurações: pasta de saída, certificados vencendo (60 dias), pendentes.
- Rodar com `cargo run -p emissor-nfse` e conferir cada tela.
- Depois: `cargo install tauri-cli` e `cargo tauri build` para o instalador MSI; CI que gera o instalador.

## Decisões já tomadas

- Interface em HTML/CSS/JS puro (sem Node/bundler), para manutenção por uma pessoa.
- `rust-version = 1.88` no workspace (subiu de 1.85 pelo `zip` do calamine, leitor de XLSX).
- Competência (`dCompet`) = data da emissão; o mês de referência vai só na descrição.
- Série própria (padrão `1`); a `70000` é do Emissor Web nacional. Pelo Anexo I (v1.01-20260209),
  a faixa 00001–49999 é de aplicativo próprio.
- Nota rejeitada é refeita com o **próximo** número ("Corrigir e emitir"). O Anexo I não exige
  `nDPS` sem buracos nem prevê inutilização de DPS; a única regra do número é a E0014 (série e
  número já usados por uma DPS que **gerou** NFS-e).
- O Id da DPS é único por ambiente (numeração de cada ambiente começa do 1).
- Inscrição municipal na DPS configurável por ambiente (E0120 quando o CNC não a tem).
- Retenções do serviço são percentuais; os valores são calculados sobre o valor de cada nota.
- XSD de Produção (09/02/2026) tem padrão de série com `^...$` (literais em XSD) e recusa tudo:
  validamos com o de Produção Restrita (`xsd/v1.01-prodrest-20260727`).
- C14N da NFS-e da Sefin: exclusivo + RSA-SHA256; nossa DPS: inclusivo + RSA-SHA1 (aceito).

## Pendências conhecidas

- Logomarca oficial da NFS-e no DANFSe (hoje há um texto no lugar): baixar do portal gov.br ou receber o PNG.
- Nome de município de tomador fora do município emissor aparece como código IBGE (falta tabela IBGE).
- Consulta de CNPJ (publica.cnpj.ws, 3 por minuto) preenche tomador e empresa; não há cache local
  nem fila — se o escritório esbarrar no limite, guardar o resultado em `pessoa` e reaproveitar.
