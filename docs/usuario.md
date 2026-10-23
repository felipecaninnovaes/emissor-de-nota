# Guia do operador

Como usar o Emissor de NFS-e no dia a dia, tela por tela. Para quem opera o sistema no dia a dia. A instalação está no [README](../README.md); as regras fiscais, em [fiscal.md](fiscal.md). O sistema não substitui a validação contábil e fiscal: alíquotas, códigos, retenções e regime são responsabilidade de quem configura (ver o [aviso no README](../README.md#aviso)).

## Primeira abertura

1. O sistema pede para **criar a senha mestra** (com confirmação, mínimo de 10 caracteres; senhas previsíveis, como só números, sequências ou "Senha@2026", são recusadas) e a marcação **Anotei a senha mestra em lugar seguro**. Ela cifra o banco inteiro e **não pode ser recuperada**: se for perdida, os dados e os certificados não abrem mais (só um backup feito antes não ajuda, porque usa a mesma senha).
2. Nas aberturas seguintes, digite a senha para destravar. O botão **Bloquear** descarta a chave e volta à tela de senha.
3. Ao abrir, o sistema mostra as notas pendentes (sem resposta da Sefin), os certificados que vencem em 30 dias e, enquanto não houver pasta de backup, o aviso **Backup desligado** (com o botão **Configurar backup**), e procura versão nova.
4. Na tela **Emitir nota**, a lista **Primeiros passos** mostra o que falta para a primeira nota, na ordem: cadastrar a empresa, importar o certificado, conferir a numeração (só para empresa importada ou banco restaurado) e cadastrar o serviço. Só o passo atual tem botão; a lista some quando tudo está pronto.

O menu lateral tem nove telas: Emitir nota, Notas emitidas, Lote do mês, Importar planilha, Recorrentes, Tomadores, Serviços, Empresas e Configurações. A empresa escolhida no topo vale para todas as telas e fica lembrada; o selo mostra o ambiente (Produção em destaque) e a validade do certificado. Em Homologação aparece também a faixa amarela **MODO TESTE** em todas as telas: as notas não têm valor fiscal.

## Cadastros

1. **Empresas → Nova empresa**: informe o CNPJ e use **Consultar** para preencher razão social, município e regime. A inscrição municipal é obrigatória (também ao editar uma empresa cadastrada sem ela). Depois importe o certificado A1 (`.pfx`) com a senha, escolhendo o arquivo em **Escolher arquivo…**. O sistema recusa certificado cujo CNPJ não seja da empresa.
2. Em **Numeração e inscrição municipal**: confira o próximo número por ambiente e, se a série já foi usada por outro emissor, use **Consultar na Sefin** para achar o primeiro número livre. A inscrição municipal pode ir ou não na DPS, por ambiente.
3. **Tomadores**: PJ com consulta de CNPJ, PF com CPF validado. O tomador é global e cada empresa o vincula, com e-mails de envio e o **Tipo de tomador** (empresa privada, órgão da União, estadual, do Distrito Federal ou município/prefeitura; só a União retém CSLL, PIS e COFINS, e para órgão público a nota leva também o tipo de ente) e as opções **Retém impostos federais** e **Retém ISS**. Na busca (em Emitir nota e em Tomadores), tomadores já cadastrados por outras empresas aparecem marcados "de outra empresa": **Vincular** (ou escolher na emissão) usa o mesmo cadastro, sem digitar de novo; os e-mails começam vazios e o tipo e as retenções vêm do cadastro feito na outra empresa; confira por empresa. A lista mostra 25 tomadores por página (**Anterior** e **Próxima** embaixo da tabela, com a contagem); a busca filtra todas as páginas. Para migrar de outro sistema, **Importar tomadores** lê uma planilha (.xlsx, .xls, .ods ou .csv; **Baixar modelo** traz as colunas): obrigatórios só CNPJ/CPF e Nome; o município vai pelo código IBGE e as retenções com Sim ou Não. Cada linha aparece com o erro encontrado, e só as marcadas são gravadas, na empresa escolhida. Tomador já cadastrado é atualizado; célula vazia mantém o que está gravado, e num vínculo novo sem retenção na planilha valem as da outra empresa.
4. **Serviços**: no topo do cadastro, **Impostos conforme o regime** preenche com o padrão do regime da empresa (só aparecem os do regime dela) o total de tributos, o PIS/COFINS próprio e o IBS/CBS: Simples Nacional (ME/EPP: % do Simples, a preencher com a alíquota da faixa), MEI (total não informado), Lucro Presumido (PIS 0,65% e COFINS 3%, CST 01, IBS/CBS informado) e Lucro Real (PIS 1,65% e COFINS 7,6%). No serviço novo de ME/EPP ou MEI ele já vem aplicado; retenções e ISS não mudam. Esses três grupos ficam recolhidos em **Opções avançadas**, com um resumo no título; um erro num campo de lá abre o bloco. O total de tributos é conferido com o regime ao salvar (ME/EPP: % do Simples; fora do Simples: percentuais; MEI: percentuais ou não informar). O tipo de retenção de PIS/COFINS/CSLL (tpRetPisCofins) não é mais escolhido: sai dos percentuais retidos preenchidos, como na emissão. Também: descrição padrão, códigos de tributação (nos campos **Cód. tributação nacional** e **NBS**, digite o código ou parte do nome, como "contabilidade", e escolha na lista que aparece, da Lista de Serviços Nacional ou da NBS; abaixo do campo aparece a descrição oficial do código), ISSQN, PIS/COFINS, retenções em percentual (cada uma vale "sempre" ou "só para tomador que retém"), total de tributos e valor sugerido. Para cadastrar outro serviço com o mesmo código e outra descrição ou outras alíquotas, use **Mais ▾ → Duplicar** na linha do serviço: o cadastro abre preenchido, com "(cópia)" no nome, e salvar cria um serviço novo sem mudar o original. Dá para **Importar serviços** de uma planilha ou de outra empresa e **Exportar planilha**. Na planilha, escreva os percentuais como número (2 para 2%; 0,65 para o PIS): célula formatada como porcentagem no Excel chega como fração (2% vira 0,02), e valores abaixo de 0,5% são recusados com essa explicação.
5. **Levar empresas para outra máquina**: em **Empresas → Exportar…**, marque as empresas e defina uma senha só para o arquivo (não a senha mestra), com pelo menos 10 caracteres. O arquivo `.nfemp` leva o cadastro, os serviços, os tomadores (com e-mails e retenções) e as recorrências. Notas, numeração e certificado ficam na máquina de origem. Na outra máquina, use **Empresas → Importar…**: escolha o arquivo, informe a senha, clique em **Ler arquivo** e marque o que importar.
   - **Empresa nova:** aparece com o selo "Conferir numeração" (o mesmo selo aparece em todas as empresas depois de restaurar um backup). Importe o certificado e use **Numeração / IM → Consultar na Sefin** antes da primeira nota, porque a outra máquina pode ter usado a série. Até lá, a emissão é recusada.
   - **Empresa já cadastrada:** os dados cadastrais, serviços, tomadores e recorrências são atualizados, sem duplicar. Ambiente, série, numeração, certificado e notas não mudam.
   - **Duas máquinas emitindo pela mesma empresa:** cada uma deve usar uma série própria.
   - **Mover todas as empresas com o histórico:** use o backup do banco (Configurações).

## Emitir uma nota

1. Escolha o tomador (busca por nome ou documento), o serviço, o valor (`1.500,00`) e o mês de referência (padrão: o mês atual, ou o anterior se escolhido em Configurações → Emissão; só muda o texto das variáveis como `{mes_ref}`, a competência da nota é a data da emissão).
2. **Revisar** mostra no topo, em quadros, o valor da nota, quanto o cliente desconta de retenções e quanto a empresa recebe. Abaixo vêm a DPS completa (os códigos técnicos, como série e IBS/CBS, ficam em **Detalhes técnicos**), cada retenção, o PIS/COFINS próprio, os códigos e a base estimada do IBS/CBS e o valor líquido (descontando o ISS retido quando o serviço tem a alíquota do ISS), além do **Ver XML da DPS** (sem assinatura) para conferir o que será enviado, e aponta erros antes de gastar número. Retenção federal com tomador pessoa física **bloqueia** a emissão (desmarque a retenção da nota). Avisos amarelos não bloqueiam: retenção federal em empresa optante pelo Simples Nacional e retenção de PIS/COFINS/CSLL em nota de até R$ 215,05.
3. **Emitir nota** envia à Sefin. Em seguida aparecem o PDF, o e-mail e **Emitir outra para este tomador** (mantém tomador e serviço) e **Tornar recorrente**.
   - **Rejeitada:** junto do erro aparece a orientação e, para as rejeições conhecidas, o botão que resolve: **Ajustar a numeração pela Sefin** (E0014; depois é só clicar de novo em **Emitir nota**), **Abrir Numeração / IM** (E0116, E0120), **Editar a empresa** (E0084, E0116), **Editar o serviço** (E0312, E0710, E0712, E0713) ou **Importar o certificado de novo** (E0714, E0717). O que foi digitado continua na tela; depois de corrigir um cadastro, revise de novo.
   - **Sem resposta definitiva:** **Verificar agora** consulta a Sefin; nunca emita de novo antes. Nos primeiros minutos a Sefin pode ainda não mostrar a nota: tente de novo depois do horário indicado.

Nota autorizada fica em `<pasta>/<EMPRESA - CNPJ>/<AAAA-MM>/` com XML e PDF.

### Variáveis na descrição

A descrição do serviço aceita variáveis que são trocadas na emissão; uma variável desconhecida (por exemplo `{tomadr}`) é barrada antes do envio.

| Variável | Resultado |
| --- | --- |
| `{mes_ref}` | `09/2026` |
| `{mes_ref_extenso}` | `setembro de 2026` |
| `{ano_ref}` | `2026` |
| `{competencia}` | mês e ano da emissão (`10/2026`) |
| `{data_emissao}` | `17/10/2026` |
| `{tomador}` | nome do tomador |
| `{empresa}` | nome da empresa |
| `{valor}` | valor da nota (`1.500,00`) |
| `{valor_liquido}` | valor menos as retenções; exige a alíquota do ISS no serviço quando o ISS é retido |

## Notas emitidas

Filtro por mês, situação de cada nota, contagem e total. A ação principal de cada linha fica visível (PDF para autorizada, **Corrigir e emitir** para rejeitada, **Verificar** para incerta) e as outras ficam em **Mais ▾**: cancelar, enviar por e-mail, **Copiar** (autorizada ou cancelada; leva tomador, serviço, valor e informações para a emissão, com a descrição refeita pelo modelo do serviço e o mês atual).

- **Corrigir e emitir**: refaz uma rejeitada com o próximo número.
- **Verificar**: consulta a Sefin para uma nota incerta (sem resposta). Nunca emita de novo antes de verificar. Nos primeiros 5 minutos depois do envio, se a Sefin ainda não mostrar a nota, ela continua incerta: verifique de novo depois do horário indicado.
- **Diagnóstico** (rejeitada, incerta ou enviando): copia para a área de transferência o erro, a resposta da Sefin e a DPS enviada, com CNPJ/CPF, nomes, endereço, telefone, e-mail e a descrição do serviço omitidos. Cole no chamado ou na conversa de suporte.
- **Cancelar**: escolha o motivo e escreva a justificativa (15 a 255 caracteres). O PDF é regerado com a marca d'água.
- **Conferir na Sefin** (em **Mais ▾**, nota autorizada) e **Conferir cancelamentos** (barra da tela, todas as autorizadas da lista): consultam se a nota foi cancelada fora do sistema, por exemplo no portal nacional. Se foi, o cancelamento é registrado aqui, com o XML do evento e o PDF com a marca d'água. O sistema não confere sozinho: depois de cancelar no portal, use um dos dois.

## Recorrentes e lote do mês

- **Recorrentes → Nova recorrência**: modelo de uma mensalidade (tomador, serviço, valor, descrição com variáveis). Se o serviço é de construção civil, o diálogo pede os dados da obra (CNO, CIB ou endereço), que vão em todas as notas da recorrência; recorrência de obra salva antes desta versão precisa ser editada para incluir a obra.
- **Lote do mês**: lista as recorrências de todas as empresas; **Marcar pendentes** e **Emitir selecionadas**. Emite uma nota por recorrência e mês, mostra o andamento e não para o lote em caso de falha. Se a Sefin limitar as requisições (429), o lote espera e tenta mais uma vez.

## Importar planilha (convênios)

Aceita XLSX, XLS, ODS e CSV, uma nota por linha; escolha o arquivo em **Escolher arquivo…** (o seletor do sistema). **Baixar modelo** dá o formato. **Ler de novo** refaz a leitura; **Marcar prontas** e **Emitir selecionadas** emitem. Reimportar a mesma planilha não repete notas já autorizadas. Se o tomador não existe, dá para cadastrá-lo direto da linha. CNPJ ou CPF digitado como número perde os zeros à esquerda; o sistema completa e, quando o número serve como CPF e como CNPJ, usa o que estiver cadastrado. Valor com mais de duas casas decimais é recusado (não é arredondado). Durante a emissão, o mês e o "Ler de novo" ficam travados.

## Configurações

- **Aparência**: tamanho do texto e densidade.
- **Emissão**: **Mês de referência sugerido**, mês atual (padrão) ou mês anterior (para quem cobra o mês que passou, como mensalidades). Vale para Emitir nota, Importar planilha, Lote do mês e Recorrentes, e também quando o campo do mês fica em branco na emissão.
- **Pasta das notas**: onde XML e PDF são gravados. Digite o caminho ou use **Escolher pasta…** (abre o seletor do sistema, começando na pasta atual); depois clique em **Salvar**. Cancelar o seletor não muda nada. O mesmo vale para a pasta do backup.
- **Backup do banco**: defina uma pasta fora da máquina (unidade externa ou pasta sincronizada). O sistema faz um backup cifrado ao abrir, no máximo uma vez por dia, e guarda os 10 mais recentes; "Fazer backup agora" força um. O backup só abre com a senha mestra. "Testar restauração" gera um banco novo a partir de um `.nfbk` e confere a integridade (o arquivo do backup e o banco a gerar podem ser digitados ou escolhidos com **Escolher arquivo…** e **Escolher destino…**), sem mexer no banco em uso. **Restaurar backup…** troca os dados em uso pelos do backup (arquivo e senha mestra do backup) e reinicia o sistema; o banco anterior fica guardado na pasta de dados como `emissor-antes-da-restauracao-<data>.db`. Na volta, abra com a senha mestra do backup: o que foi feito depois dele não está no sistema, e todas as empresas ficam com "Conferir numeração" até a consulta à Sefin em **Numeração / IM**.
- **Diagnóstico**: versão, sistema, leiautes da DPS e as últimas linhas do log, com **Copiar diagnóstico** e **Abrir pasta de logs**. O log é um arquivo por dia (os 14 mais recentes) com o ciclo de cada emissão e as respostas da Sefin, sem senhas nem o XML completo. **Log detalhado** acrescenta eventos internos (use só para investigar um problema).
- **E-mail das notas**: servidor SMTP da conta que envia as notas; **Enviar teste**. Pode enviar sozinho depois de cada emissão. Segurança "Nenhuma" (sem cifra) só serve para servidor da rede local sem login: com usuário preenchido é recusada, porque a senha iria aberta pela rede.
- **Certificados vencendo** (60 dias) e **Envios sem resposta da Sefin**.
- **Atualizações**: **Procurar atualizações** e **Atualizar agora**.
- **Código-fonte**: endereço do código da versão instalada (licença AGPL-3.0).

## Atalhos

| Atalho | Ação |
| --- | --- |
| `Alt` + `1` … `9` | Ir para a tela, na ordem do menu |
| `Ctrl` + `N` | Nova nota |
| `Ctrl` + `Enter` | Na emissão, revisa; com a revisão aberta, emite; nos diálogos, salva |
| `Enter` | Nos diálogos, salva; no campo de CNPJ, consulta |
| `Ctrl` + `F` | Busca do tomador (em Emitir nota e em Tomadores) |
| `F5` | Atualiza a lista da tela |
| `Esc` | Fecha listas, menus e diálogos (pede confirmação se houver alteração) |
| `Ctrl` + `+` / `−` / `0` | Tamanho do texto |
