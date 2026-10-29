# Changelog

Histórico das versões do aplicativo (`emissor-nfse`), reconstruído a partir das tags e dos commits. Para quem acompanha o que mudou entre instalações. As versões até a 0.2.9 saíram em 17/10/2026; a 1.0.0, a 1.1.0, a 1.2.0, a 1.3.0 e a 1.4.0, em 18/10/2026, a 1.5.0 e a 1.6.0, em 19/10/2026, a 1.7.0 e a 1.7.1, em 20/10/2026, a 1.8.0 e a 1.8.1, em 22/10/2026, a 1.9.0, em 22/10/2026, a 1.10.0, em 23/10/2026, a 1.11.0 e a 1.11.1, em 26/10/2026, a 1.12.0, em 27/10/2026, e a 1.13.0 e a 1.14.0, em 28/10/2026; a versão vem de `crates/app/Cargo.toml`. A seção **Não publicado** junta o que já está na `main` e ainda não saiu em versão.

## Não publicado

- Retenção de até R$ 10,00: a revisão tem caixas para reter mesmo assim o IRRF ou PIS/COFINS/CSLL nesta nota, quando outras notas do mesmo período, somadas, passam do limite.

## 1.14.0

Dispensa de retenção federal de até R$ 10,00. Sem nova migração nem mudança no esquema do banco.

- Retenção de até R$ 10,00 é dispensada: o IRRF (com 1,5%, nota de até R$ 666,66) e PIS, COFINS e CSLL somados (com 4,65%, nota de até R$ 215,05) deixam de ser retidos, e a revisão avisa o que foi dispensado.

## 1.13.0

Substituição de nota autorizada. Sem nova migração nem mudança no esquema do banco.

- Substituição de nota: em Notas emitidas, "Substituir nota" (no menu Mais de uma nota autorizada) leva os dados para a emissão com o motivo da substituição; a nova nota sai com o próximo número e, autorizada, a original passa a "Substituída", com o PDF marcado.

## 1.12.0

Backup na nuvem, para quem contrata o serviço. Sem nova migração nem mudança no esquema do banco.

- Backup na nuvem (serviço contratado à parte): em Configurações → Backup do banco, informe o endereço do servidor e o token; o sistema envia um backup cifrado ao abrir (no máximo uma vez por dia) ou em "Enviar agora", lista os backups guardados no servidor e restaura qualquer um deles com a senha mestra. Com a assinatura inativa, os envios param, mas a restauração continua.

## 1.11.1

Mesmo conteúdo da 1.11.0, cujos instaladores não chegaram a ser publicados (falha no build do macOS). Sem nova migração nem mudança no esquema do banco.

- Corrigido o build dos instaladores do macOS, que parava ao instalar as ferramentas de empacotamento.

## 1.11.0

PIS e COFINS retidos aparecem no DANFSe como débito de apuração própria. Sem nova migração nem mudança no esquema do banco.

- Nota com PIS/COFINS retidos passa a levar também a base, as alíquotas e os valores de PIS e COFINS: o DANFSe deixava zerados "PIS - Débito Apuração Própria" e "COFINS - Débito Apuração Própria", e a base do IBS/CBS não descontava esses valores.
- Corrigida a mensagem de nota autorizada (e a de nota não emitida) na tela de emissão, que espalhava o título, "Chave:" e a chave em colunas desalinhadas.

## 1.10.0

Importação de tomadores por planilha, paginação de tomadores e duplicação de serviço. Sem nova migração nem mudança no esquema do banco.

- A lista de **Tomadores** passa a mostrar todos os tomadores da empresa, 25 por página, com **Anterior**/**Próxima** e a contagem embaixo da tabela; antes parava nos 20 primeiros e só a busca achava os outros.
- **Serviços → Mais ▾ → Duplicar** abre o cadastro preenchido com os dados de um serviço já cadastrado (nome com "(cópia)"); salvar cria outro serviço, com o mesmo código de tributação e a descrição ou as alíquotas que forem trocadas, sem mudar o original.
- Tomador vinculado de outra empresa passa a vir com o tipo e as retenções (**Retém impostos federais**, **Retém ISS**) do cadastro feito na outra empresa, em vez de vazios.
- A **inscrição municipal** passa a ser obrigatória no cadastro da empresa (também ao editar uma empresa salva sem ela).
- **Tomadores → Importar tomadores**: lê uma planilha (.xlsx, .xls, .ods ou .csv) com CNPJ/CPF, nome, inscrição municipal, endereço, contatos, tipo e retenções, mostra cada linha com o erro encontrado e grava os marcados na empresa, para migrar o cadastro de outro sistema. Tomador já cadastrado é atualizado e célula vazia mantém o que está gravado; **Baixar modelo** traz as colunas aceitas.
- Nota sem tomador deixa de aceitar retenção: a revisão não mostra mais as caixas **Reter nesta nota** e a validação recusa retenção federal ou ISS retido antes de reservar o número, em vez da rejeição da Sefin.

## 1.9.0

Diagnóstico e log para investigar rejeições da Sefin. Sem nova migração; o banco ganha a coluna `dps.resposta_sefin` por ajuste de esquema ao abrir.

- Corrigido o botão **Diagnóstico** que vazava da coluna de ações nas notas rejeitadas (agora fica em **Mais ▾**) e a barra de rolagem vertical que aparecia em várias telas sem haver o que rolar.
- **Configurações → Diagnóstico**: versão, sistema, leiautes da DPS e as últimas linhas do log, com **Copiar diagnóstico**, **Abrir pasta de logs** e **Log detalhado**. O aplicativo passa a gravar um log por dia (14 dias) com o ciclo da emissão e as respostas da Sefin, sem senhas nem o XML completo.
- **Notas → Diagnóstico** (rejeitada, incerta ou enviando): copia o erro, a resposta da Sefin e a DPS enviada, com documentos, nomes, endereço e descrição do serviço omitidos. A resposta da Sefin fica guardada no banco (coluna `dps.resposta_sefin`, sem nova migração).
- A rejeição **E1235** passa a orientar a troca do leiaute da DPS e o **E0617** explica a alíquota para não optante.

## 1.8.1

Correção da DPS para prestador que não é optante do Simples. Sem mudança de esquema do banco.

- A DPS deixa de informar a alíquota do ISS (`pAliq`) quando o prestador não é optante do Simples Nacional: a Sefin calcula a alíquota pelo parâmetro do município ativo no Sistema Nacional e recusava a nota com E0617.

## 1.8.0

DPS no leiaute da NT 009, com escolha do leiaute por ambiente, e correção da soma das retenções federais. Sem mudança de esquema do banco.

**Emissão**

- A DPS sai no leiaute da **NT 009**, que a Produção Restrita passou a exigir (rejeição E1235): o `finNFSe` sai do grupo IBS/CBS e vai para `infDPS`, entre `tpEmit` e `cLocEmi`; o `cNBS` vem antes de `xDescServ`; CST e `cClassTrib` ficam direto em `IBSCBS/valores/trib`.
- **Configurações → Leiaute da DPS** escolhe o leiaute por ambiente, porque a Produção pode receber a NT 009 depois da Produção Restrita. Padrão: NT 009 na Produção Restrita e NT 004 na Produção.
- O PDF da nota lê os campos nos dois leiautes.
- Embutido o XSD de Produção Restrita de 20/10/2026 (NT 009); o pacote anterior continua para validar o leiaute NT 004.

**Retenções**

- Quando o tipo de retenção do cadastro marca PIS e/ou COFINS como retidos (sem percentual de retenção no serviço), os valores entram também na soma de `vRetCSLL`, como na nota do Portal Nacional; `vPis` e `vCofins` continuam na DPS.

**Geral**

- O workflow do instalador deixa de rodar os testes (o CI da `main` já cobre) e aquece o cache das dependências em push na `main` que altere o `Cargo.lock`.

## 1.7.1

Busca de tomador e de CEP mais fácil. Sem mudança de esquema do banco.

- A busca de tomadores ignora maiúsculas e acentos ("saude" acha "Saúde").
- Botão **Não sei o CEP** ao lado do CEP do tomador, da obra na emissão e da obra da recorrência: pede UF, cidade e rua, consulta o ViaCEP e lista os CEPs; escolher um preenche o campo e completa o endereço. Só UF, cidade e rua saem da máquina nessa busca.
- Depois de autorizar a nota, a tela limpa também o valor; se o serviço tem valor sugerido, ele volta preenchido.

## 1.7.0

Mais fácil para quem começa: primeiros passos, cadastro de serviço por regime, correções direto da rejeição e restauração de backup pelo sistema. Sem mudança de esquema do banco.

**Emissão**

- **Primeiros passos** na tela Emitir nota: cadastrar a empresa, importar o certificado, conferir a numeração (empresa importada ou banco restaurado) e cadastrar o serviço, cada passo com o botão que resolve. Sem empresa cadastrada, o sistema abre nessa tela.
- A revisão mostra no topo o valor da nota, quanto o cliente desconta de retenções e quanto a empresa recebe; série e códigos do IBS/CBS foram para **Detalhes técnicos**.
- Nota rejeitada ganha o botão da correção conforme o código: **Ajustar a numeração pela Sefin** (E0014; depois é só emitir de novo), **Abrir Numeração / IM** (E0116, E0120), **Editar a empresa** (E0084, E0116), **Editar o serviço** (E0312, E0710, E0712, E0713) e **Importar o certificado de novo** (E0714, E0717).
- Envio sem resposta definitiva da Sefin ganha **Verificar agora** na própria tela.
- Corrigir o cadastro da empresa ou do serviço não apaga mais o que foi digitado na emissão; só a revisão precisa ser refeita.
- **Mês de referência sugerido** configurável em Configurações → Emissão: mês atual (novo padrão) ou mês anterior (como antes, para mensalidades). Vale para Emitir nota, Importar planilha, Lote do mês e Recorrentes. A competência da nota continua sendo a data da emissão.

**Serviços**

- **Impostos conforme o regime** no topo do cadastro: Simples Nacional (ME/EPP), MEI, Lucro Presumido ou Lucro Real preenchem o total de tributos, o PIS/COFINS próprio e o IBS/CBS. Só aparecem os do regime da empresa; retenções e ISS não mudam.
- Total de tributos, PIS/COFINS próprio e IBS/CBS ficam em **Opções avançadas**, com resumo no título.
- A forma do total de tributos é conferida com o regime ao salvar, antes de virar rejeição E0710, E0712 ou E0713.
- O tipo de retenção de PIS/COFINS/CSLL deixou de ser escolhido numa lista de dez opções: sai dos percentuais retidos, como já acontecia na emissão.

**Segurança dos dados**

- Faixa **MODO TESTE** em todas as telas quando a empresa emite em Homologação.
- Criar a senha mestra exige marcar "Anotei a senha mestra em lugar seguro".
- Aviso **Backup desligado** no topo enquanto não houver pasta de backup.
- **Restaurar backup…** em Configurações: troca os dados em uso pelos do backup e reinicia o sistema; o banco anterior fica guardado na pasta de dados. Depois da restauração, todas as empresas exigem conferir a numeração na Sefin antes de emitir.

**Geral**

- Licença trocada de MIT para AGPL-3.0 (ADR 0016); Configurações mostra o endereço do código-fonte da versão instalada.
- Textos e exemplos genéricos, sem referência a escritório de contabilidade (cadastro de serviço, e-mail, modelo de planilha, documentação).

## 1.6.0

Retenções por tipo de tomador (órgão público), revisão da nota mais completa e campos de ente governamental na DPS. Muda o esquema do banco (migração `0009`).

- **Tipo de tomador** no cadastro (empresa privada, órgão da União, estadual, do Distrito Federal ou município/prefeitura). Para estado, Distrito Federal e município, a nota retém só o IRRF e o INSS: a CSLL, o PIS e a COFINS do serviço são ignorados, porque só órgãos da União retêm as contribuições. Para órgão público a DPS leva também `tpEnteGov` e `tpOper` (1), como o Portal Nacional, e `indFinal` passa a ir sempre. Exige a migração `0009`; os tomadores existentes ficam como privado e o arquivo de empresas (`.nfemp`) leva o tipo (issue #2).
- A revisão da nota (**Revisar**) mostra cada retenção (IRRF, INSS, CSLL/PIS/COFINS), o PIS e a COFINS de apuração própria, os códigos do IBS/CBS com a base estimada o valor líquido já descontando o ISS retido quando o serviço tem a alíquota do ISS e o **Ver XML da DPS** (sem assinatura, com número e hora provisórios).
- No cadastro do serviço, "Federais e retenções" foi separado em **PIS e COFINS apuração própria** e **Retenções na fonte**, com explicação em cada um e um aviso quando PIS ou COFINS estão preenchidos como próprios e como retidos (vale só o retido).
- Corrigido o texto `undefined so_tomador_que_retem%` na coluna de retenções da lista de serviços.

## 1.5.0

Reforma Tributária (IBS/CBS) no cadastro de serviços e PIS/COFINS de apuração própria na nota (sem mudança de esquema do banco).

**IBS/CBS**

- O `cClassTrib` e o `cIndOp` viraram campos com busca por código ou nome, nas tabelas oficiais do Portal DFe (SVRS) embutidas no aplicativo (173 classificações tributárias e 39 indicadores da operação). A descrição do `cClassTrib` mostra o CST e a redução de alíquota (por exemplo, `200029`, serviços de saúde humana: CST 200, redução de 60% no IBS e na CBS).
- Escolher o `cClassTrib` preenche o CST. CST que não é o do `cClassTrib` agora é recusado no cadastro, na importação por planilha e na emissão; na planilha, CST em branco assume o da tabela. Código fora da tabela só gera aviso.
- O `cIndOp` deixou de ser uma lista de 11 opções: o padrão de serviço novo continua `100301` (demais serviços); consulta médica presencial usa `030101`.

**PIS/COFINS**

- Novos campos "PIS próprio %" e "COFINS próprio %" no serviço. Com CST 01 ou 02 e sem retenção de PIS/COFINS, a nota leva a base, as alíquotas e os valores de PIS e COFINS, truncados no centavo como no Emissor Web. A Sefin desconta esses valores da base do IBS/CBS, que antes saía só menos o ISS. Serviços já cadastrados continuam sem alíquota até serem editados.

**Empresas**

- A opção de regime especial 6 passou a se chamar "Sociedade de profissionais (ISS fixo)", com texto de ajuda.

## 1.4.0

Exportação de empresas entre máquinas e seletores de arquivo do sistema. O banco ganha a migração 0008 (coluna `empresa.conferir_numeracao`), aplicada sozinha ao abrir.

**Empresas**

- **Exportar…** gera um arquivo `.nfemp` com as empresas marcadas, seus serviços, tomadores (com e-mails e retenções) e recorrências, cifrado com uma senha própria do arquivo (não a senha mestra). Notas, numeração e certificado ficam na máquina de origem (issue #1, ADR 0015).
- **Importar…** lê o arquivo, mostra o que é empresa nova, já cadastrada ou arquivada e importa as marcadas, tudo ou nada e sem duplicar tomadores, serviços e recorrências. Empresa já cadastrada mantém ambiente, série, numeração, certificado e notas.
- Empresa nova importada aparece com o selo "Conferir numeração" e só emite depois de **Numeração / IM → Consultar na Sefin**, porque a outra máquina pode ter usado a série.

**Arquivos**

- Certificado A1, planilha de notas e planilha de serviços passam a usar o seletor de arquivos do sistema (no Linux, pelo xdg-desktop-portal, com o diálogo do KDE ou do GNOME); o campo mostra o nome do arquivo escolhido (issue #8).
- Testar restauração de backup ganhou **Escolher arquivo…** para o `.nfbk` e **Escolher destino…** para o banco a gerar.

## 1.3.0

Busca de códigos no cadastro de serviços (sem mudança de esquema do banco).

**Serviços**

- O campo Cód. tributação nacional aceita o código ou parte do nome e mostra a lista para escolher, com a descrição oficial embaixo; o campo separado de busca saiu.
- O campo NBS ganhou a mesma busca, com a tabela de códigos NBS de 9 dígitos embutida a partir do Anexo B (issue #9).
- Sair do campo não apaga mais um nome digitado; ao salvar, continua exigindo 6 dígitos no código nacional e 9 na NBS.

## 1.2.0

Robustez do banco e da interface, e reorganização interna do código (sem mudança de esquema do banco).

**Banco**

- Empresas, certificado, numeração, importação de serviços e tomador com vínculo são gravados numa transação só, com a auditoria dentro dela.
- A conferência de envios pendentes e a leitura da empresa ao reservar número acontecem na mesma transação da escrita.
- Alterar serviço ou recorrência confere a empresa dona do cadastro.

**Emissão e DANFSe**

- Cálculo de percentual sobre valor sem risco de estouro; valores negativos aparecem com sinal.
- O DANFSe confere o formato dos campos antes de formatá-los, em vez de falhar com dado inesperado.
- A importação de planilha só vincula o tomador à empresa quando ele ainda não está vinculado.
- Busca de serviço pelo nome na importação acha nomes com qualquer acento (è, ì, ò, ä…).

**Interface**

- Busca e Revisar descartam respostas antigas; Revisar não dispara duas vezes e mostra o erro junto do campo.
- Totais do lote e da planilha somados em centavos.
- O lote só reenvia automaticamente quando a Sefin responde 429 (limite de requisições).
- Atualização do sistema bloqueada durante emissão ou cancelamento.
- "Marcar prontas" na importação de planilha fica desligado durante o envio, como no lote.

**Código**

- Interface dividida em módulos ES nativos (`crates/app/ui/js/`), ainda sem bundler; `nfse-emissor` dividido em submódulos; normalização de texto única no `nfse-core` (issue #10).

## 1.1.0

Correções da revisão do projeto (emissão, segurança, planilhas e interface) e conferência de cancelamentos feitos fora do sistema.

**Atenção ao atualizar**

- O banco ganha a coluna `recorrencia.obra_json` (migração `0007`) e `dps.enviado_em` na primeira abertura. Depois de aberto na 1.1.0, não volte para a 1.0.0.
- E-mail com segurança "Nenhuma" (sem cifra) e usuário preenchido deixa de enviar: troque para STARTTLS ou SSL em Configurações.
- Recorrências de serviço de construção civil precisam ser editadas para incluir os dados da obra.

**Emissão**

- NFS-e autorizada pela Sefin que não pôde ser gravada localmente fica **incerta** (com a chave no motivo) e é buscada de novo na reconciliação, em vez de aparecer como erro e permitir emitir outra.
- Um 404 logo após um envio sem resposta não libera mais a DPS na hora: ela continua incerta por 5 minutos (`JANELA_RECONCILIACAO`), porque a Sefin pode ainda não mostrá-la. "Verificar" num envio ainda em curso é recusado. Resposta de sucesso sem `chaveAcesso` passa a ser erro de formato.
- DPS que ficava presa em `reservada` (falha entre a reserva e o envio, ou programa fechado) passa a `rejeitada` e não bloqueia mais a recorrência ou a linha da planilha.
- Autorização e cancelamento são gravados numa transação só, com a auditoria.
- Textos fora do padrão do XSD (travessão ou aspas curvas coladas de outro programa, quebra de linha, espaço nas pontas) em endereço, e-mail, informações complementares e justificativa são recusados na revisão, antes de reservar número.

**Cancelamento**

- **Conferir na Sefin** (Mais ▾) e **Conferir cancelamentos** (Notas emitidas): registram cancelamentos feitos no portal nacional, com o XML do evento e o PDF com a marca d'água.
- Cancelamento recusado ou sem resposta consulta o evento antes de dar erro: nota já cancelada no portal passa a cancelada.

**Recorrências**

- Dados da obra (CNO, CIB ou endereço) na recorrência, para serviço de construção civil (regra E0370); **Tornar recorrente** leva a obra da nota.

**Planilhas**

- Percentual de célula formatada como % no Excel (2% chega como 0,02) é apontado na importação de serviços, em vez de entrar como 0,02%. Valor com mais de duas casas é recusado, não arredondado.
- Número que pode ser CPF ou CNPJ usa o documento cadastrado.

**Interface**

- Durante a emissão em lote ou da planilha, o mês, os filtros e o "Ler de novo" ficam travados; um erro no meio não deixa mais o botão preso.
- A revisão fecha quando o formulário muda, para o "Emitir nota" (e o Ctrl+Enter) não enviar o pedido antigo.
- Depois de salvar num diálogo, a recarga da lista não reabre o diálogo com erro (evitava e-mail e cadastro duplicados).
- A tela de criação da senha mestra avisa que ela cifra o banco, os certificados e os backups, sem recuperação.

**Segurança**

- Senha mestra de cofre novo: mínimo de 10 caracteres, e senhas previsíveis (só números, sequências, "Senha@2026") são recusadas. Cofres novos usam Argon2id com 64 MiB e 3 passadas; os existentes mantêm os parâmetros.
- Restaurar backup recusa destino que sobrescreveria o `emissor.cofre` em uso.
- Cliente da Sefin sem redirecionamentos e com TLS 1.2 no mínimo; chave de acesso conferida antes de virar nome de arquivo; XML descompactado limitado a 10 MiB; cabeçalho de backup com custo de Argon2 exagerado é recusado.
- CSP sem `'unsafe-inline'`; senhas e chave do banco zeradas da memória depois do uso.

**Diagnóstico**

- `nfse-poc consultar-eventos`: mostra a resposta bruta da consulta de eventos de uma NFS-e.

## 1.0.0

Primeira versão estável: emissão, cancelamento, consultas, cadastros, recorrências, backup cifrado e atualização automática nas quatro plataformas.

- Serviços: busca do código de tributação nacional (cTribNac) pelo nome, com a Lista de Serviços Nacional embutida.
- Linux: o **Escolher pasta…** usa o xdg-desktop-portal e abre o diálogo do ambiente (KDE, GNOME), não o do GTK.
- Linux/KDE com Wayland: o app roda pelo XWayland para a barra de título ser a do KWin (Breeze); `GDK_BACKEND=wayland` desfaz ([configuracao.md](docs/configuracao.md#variáveis-de-ambiente)).

## 0.2.9

- Configurações: botão **Escolher pasta…** para a pasta das notas e a do backup, com o seletor de pastas do sistema.

## 0.2.8

- **Instaladores para macOS e Linux:** a Release passa a trazer DMG e `.app.tar.gz` (Apple Silicon e Intel), AppImage e DEB, além do MSI. O `latest.json` cobre as quatro plataformas e o atualizador funciona em todas (no Linux, só para o AppImage). O macOS ainda não é assinado pela Apple ([deploy.md](docs/deploy.md)).
- macOS: a pasta de dados deixa de ter o sufixo `.app`; a pasta antiga é migrada.

## 0.2.7

- **Banco cifrado em repouso** (SQLCipher): o `emissor.db` inteiro passa a ser cifrado com a chave da senha mestra. O sal e o verificador ficam em `emissor.cofre`, ao lado do banco ([ADR 0013](docs/decisoes/0013-banco-cifrado-sqlcipher.md)).
- **Atualização de bancos existentes:** na primeira abertura com a senha mestra o banco é convertido sem perder dados nem reimportar certificados. O original em claro fica como `emissor.db.legado`; **apague-o depois de conferir**, porque ele continua legível.
- Copiar o banco para outra máquina exige copiar `emissor.db` e `emissor.cofre` juntos. O backup `.nfbk` leva o cabeçalho e a restauração gera os dois arquivos.
- A aparência (zoom e densidade) passa a ficar em `aparencia.json`, fora do banco; volta ao padrão uma vez.

## 0.2.6

- **Backup cifrado do banco** (`.nfbk`), automático ao abrir o sistema e manual, com teste de restauração; guarda as 10 cópias mais recentes.
- Tomador pessoa física não retém tributos federais: a emissão com essa retenção é bloqueada na validação.
- Revisão avisa de retenção federal para optante do Simples Nacional e em nota de até R$ 215,05.
- Vincular um tomador já cadastrado por outra empresa, sem recadastrar.
- Documentação reorganizada (README, guia do operador, ADRs) e aviso de responsabilidade fiscal.

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
