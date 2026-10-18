# Guia do operador

Como usar o Emissor de NFS-e no dia a dia, tela por tela. Para quem opera o sistema no escritório. A instalação está no [README](../README.md); as regras fiscais, em [fiscal.md](fiscal.md). O sistema não substitui a validação contábil e fiscal: alíquotas, códigos, retenções e regime são responsabilidade de quem configura (ver o [aviso no README](../README.md#aviso)).

## Primeira abertura

1. O sistema pede para **criar a senha mestra** (com confirmação, mínimo de 10 caracteres; senhas previsíveis, como só números, sequências ou "Senha@2026", são recusadas). Ela cifra o banco inteiro e **não pode ser recuperada**: se for perdida, os dados e os certificados não abrem mais (só um backup feito antes não ajuda, porque usa a mesma senha).
2. Nas aberturas seguintes, digite a senha para destravar. O botão **Bloquear** descarta a chave e volta à tela de senha.
3. Ao abrir, o sistema mostra as notas pendentes (sem resposta da Sefin) e os certificados que vencem em 30 dias, e procura versão nova.

O menu lateral tem nove telas: Emitir nota, Notas emitidas, Lote do mês, Importar planilha, Recorrentes, Tomadores, Serviços, Empresas e Configurações. A empresa escolhida no topo vale para todas as telas e fica lembrada; o selo mostra o ambiente (Produção em destaque) e a validade do certificado.

## Cadastros

1. **Empresas → Nova empresa**: informe o CNPJ e use **Consultar** para preencher razão social, município e regime. Depois importe o certificado A1 (`.pfx`) com a senha. O sistema recusa certificado cujo CNPJ não seja da empresa.
2. Em **Numeração e inscrição municipal**: confira o próximo número por ambiente e, se a série já foi usada por outro emissor, use **Consultar na Sefin** para achar o primeiro número livre. A inscrição municipal pode ir ou não na DPS, por ambiente.
3. **Tomadores**: PJ com consulta de CNPJ, PF com CPF validado. O tomador é global e cada empresa o vincula, com e-mails de envio e as opções **Retém impostos federais** e **Retém ISS**. Na busca (em Emitir nota e em Tomadores), tomadores já cadastrados por outras empresas aparecem marcados "de outra empresa": **Vincular** (ou escolher na emissão) usa o mesmo cadastro, sem digitar de novo; e-mails e retenções começam vazios e são conferidos por empresa.
4. **Serviços**: descrição padrão, códigos de tributação (sem saber o código, use **Procurar o código pelo nome**: digite parte do nome, como "contabilidade", e escolha na Lista de Serviços Nacional; abaixo da busca aparece a descrição oficial do código), ISSQN, PIS/COFINS, retenções em percentual (cada uma vale "sempre" ou "só para tomador que retém"), total de tributos e valor sugerido. Dá para **Importar serviços** de uma planilha ou de outra empresa e **Exportar planilha**. Na planilha, escreva os percentuais como número (2 para 2%; 0,65 para o PIS): célula formatada como porcentagem no Excel chega como fração (2% vira 0,02), e valores abaixo de 0,5% são recusados com essa explicação.

## Emitir uma nota

1. Escolha o tomador (busca por nome ou documento), o serviço, o valor (`1.500,00`) e o mês de referência (padrão: o mês anterior).
2. **Revisar** mostra a DPS completa, as retenções e o valor líquido, e aponta erros antes de gastar número. Retenção federal com tomador pessoa física **bloqueia** a emissão (desmarque a retenção da nota). Avisos amarelos não bloqueiam: retenção federal em empresa optante pelo Simples Nacional e retenção de PIS/COFINS/CSLL em nota de até R$ 215,05.
3. **Emitir nota** envia à Sefin. Em seguida aparecem o PDF, o e-mail e **Emitir outra para este tomador** (mantém tomador e serviço) e **Tornar recorrente**.

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
- **Cancelar**: escolha o motivo e escreva a justificativa (15 a 255 caracteres). O PDF é regerado com a marca d'água.
- **Conferir na Sefin** (em **Mais ▾**, nota autorizada) e **Conferir cancelamentos** (barra da tela, todas as autorizadas da lista): consultam se a nota foi cancelada fora do sistema, por exemplo no portal nacional. Se foi, o cancelamento é registrado aqui, com o XML do evento e o PDF com a marca d'água. O sistema não confere sozinho: depois de cancelar no portal, use um dos dois.

## Recorrentes e lote do mês

- **Recorrentes → Nova recorrência**: modelo de uma mensalidade (tomador, serviço, valor, descrição com variáveis). Se o serviço é de construção civil, o diálogo pede os dados da obra (CNO, CIB ou endereço), que vão em todas as notas da recorrência; recorrência de obra salva antes desta versão precisa ser editada para incluir a obra.
- **Lote do mês**: lista as recorrências de todas as empresas; **Marcar pendentes** e **Emitir selecionadas**. Emite uma nota por recorrência e mês, mostra o andamento e não para o lote em caso de falha. Se a Sefin limitar as requisições (429), o lote espera e tenta mais uma vez.

## Importar planilha (convênios)

Aceita XLSX, XLS, ODS e CSV, uma nota por linha. **Baixar modelo** dá o formato. **Ler de novo** refaz a leitura; **Marcar prontas** e **Emitir selecionadas** emitem. Reimportar a mesma planilha não repete notas já autorizadas. Se o tomador não existe, dá para cadastrá-lo direto da linha. CNPJ ou CPF digitado como número perde os zeros à esquerda; o sistema completa e, quando o número serve como CPF e como CNPJ, usa o que estiver cadastrado. Valor com mais de duas casas decimais é recusado (não é arredondado). Durante a emissão, o mês e o "Ler de novo" ficam travados.

## Configurações

- **Aparência**: tamanho do texto e densidade.
- **Pasta das notas**: onde XML e PDF são gravados. Digite o caminho ou use **Escolher pasta…** (abre o seletor do sistema, começando na pasta atual); depois clique em **Salvar**. Cancelar o seletor não muda nada. O mesmo vale para a pasta do backup.
- **Backup do banco**: defina uma pasta fora da máquina (unidade externa ou pasta sincronizada). O sistema faz um backup cifrado ao abrir, no máximo uma vez por dia, e guarda os 10 mais recentes; "Fazer backup agora" força um. O backup só abre com a senha mestra. "Testar restauração" gera um banco novo a partir de um `.nfbk` e confere a integridade; para restaurar de verdade, feche o sistema e troque o `emissor.db` pelo arquivo gerado.
- **E-mail das notas**: servidor SMTP do escritório; **Enviar teste**. Pode enviar sozinho depois de cada emissão. Segurança "Nenhuma" (sem cifra) só serve para servidor da rede local sem login: com usuário preenchido é recusada, porque a senha iria aberta pela rede.
- **Certificados vencendo** (60 dias) e **Envios sem resposta da Sefin**.
- **Atualizações**: **Procurar atualizações** e **Atualizar agora**.

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
