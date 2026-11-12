# 0001 — Importação de pedidos de nota por e-mail

- **Status:** aprovada <!-- rascunho | aprovada | implementada | abandonada -->
- **Autor:** Felipe CN
- **Data:** 2026-11-12
- **ADRs relacionados:** [0013](../../decisoes/0013-banco-cifrado-sqlcipher.md) (dados locais cifrados), [0017](../../decisoes/0017-backup-na-nuvem.md) (o servidor não lê dados)

## Contexto e problema
Hospitais, convênios e clínicas pedem as notas por e-mail, e o operador redigita tudo no Emitir nota: tomador, valor, período e descrição. Cada nota redigitada é trabalho repetido e uma chance de errar o CPF ou o valor. Há dois tipos de e-mail:

- **Texto corrido, uma nota:** o tomador (PJ, às vezes uma filial) aparece pelo CNPJ no meio da frase, junto do valor ("R$ …") e do período ("MM/AAAA"). O período costuma ser de meses antes do envio.
- **Blocos com rótulos, várias notas:** um bloco por paciente (pessoa física, muitas vezes sem cadastro), com rótulos como PACIENTE, CPF, ENDEREÇO, BAIRRO, CEP, CIDADE, VALOR e "colocar no corpo da nota". O leiaute é irregular: há vários rótulos na mesma linha, valores quebrados em duas linhas, espaços sobrando e campos que só aparecem às vezes.

O escritório usa o Outlook clássico, que salva em `.msg`, o novo Outlook e o webmail, que salvam em `.eml`, e texto repassado por outros meios.

## Objetivos
- Transformar um e-mail salvo ou colado em linhas de nota na tela **Importar planilha**, para conferir e emitir pelo fluxo que já existe.
- Cadastrar ao emitir o paciente novo que vem no e-mail, sem digitar de novo.

## Fora de escopo
- Ler a caixa de e-mail (IMAP, Microsoft Graph, Gmail API) ou arrastar a mensagem direto do Outlook. Nesta versão a entrada é só por arquivo escolhido ou texto colado.
- Configuração por remetente (empresa, serviço ou modelo de descrição por endereço de e-mail) e regras de extração configuráveis.
- Dados em anexo (PDF, planilha anexada). Os anexos são ignorados.
- Corpo só em RTF compactado no `.msg`.
- Responder ao remetente com a nota ou sugerir o remetente como destinatário do envio.
- IA ou serviço externo para interpretar o texto.
- Emitir sem revisão: nada é emitido sem o operador marcar a linha.
- Aba nova no menu.

## Requisitos
| ID | Requisito | Prioridade |
|---|---|---|
| R1 | A tela Importar planilha aceita, além das planilhas, arquivos `.eml` e `.msg` pelo mesmo **Escolher arquivo…**. | obrigatório |
| R2 | A tela tem um campo para colar o texto do e-mail, com um botão para ler o texto colado. | obrigatório |
| R3 | Do `.eml` (MIME) o app lê o corpo em texto puro. Sem texto puro, usa o HTML convertido em texto. Decodifica quoted-printable, base64 e o charset declarado (UTF-8, ISO-8859-1, Windows-1252). | obrigatório |
| R4 | Do `.msg` (contêiner CFB do Outlook) o app lê o corpo em texto, em Unicode ou na página de código declarada. Sem texto, usa o HTML convertido em texto. Com corpo só em RTF, mostra "não consegui ler o corpo deste e-mail; cole o texto". | obrigatório |
| R5 | No leiaute de **blocos com rótulos**, cada bloco iniciado por PACIENTE vira uma linha com nome, CPF, endereço, bairro, CEP, cidade/UF, valor e descrição ("colocar no corpo da nota"). Os rótulos são reconhecidos sem acento, sem diferença de maiúsculas e com qualquer espaço em volta. Pode haver mais de um rótulo por linha de texto, e o valor de um rótulo pode continuar na linha seguinte até o próximo rótulo. | obrigatório |
| R6 | No leiaute de **texto corrido** (sem rótulo PACIENTE), o e-mail vira uma linha. Tomador: o primeiro CNPJ ou CPF com dígito verificador válido que não seja o CNPJ de uma empresa cadastrada. Valor: o primeiro após "R$". Período: o primeiro "MM/AAAA". | obrigatório |
| R7 | Mês de referência: o período encontrado no e-mail. Sem período, vale o mês escolhido na tela, como na planilha. | obrigatório |
| R8 | Ao ler um e-mail, a tela exige um serviço da empresa escolhida, que vale para todas as linhas dele. Sem serviço escolhido, as linhas ficam com o erro "escolha o serviço". | obrigatório |
| R9 | Descrição da nota: o texto de "colocar no corpo da nota" do bloco, se houver; senão, a descrição padrão do serviço. As variáveis (`{mes}`, `{pix}` etc.) valem nos dois casos, como hoje. | obrigatório |
| R10 | Tomador já cadastrado (pelo CPF/CNPJ) é usado como está; o cadastro não muda. Se o nome ou o CEP do e-mail diferem do cadastro, a linha mostra um aviso (não é erro). O vínculo com a empresa é criado ao emitir, como na planilha. | obrigatório |
| R11 | Pessoa física sem cadastro vira "tomador novo" na linha, com nome, CPF e endereço do e-mail. O município (código IBGE) vem da consulta do CEP (ViaCEP), que o app já usa. A pessoa é cadastrada e vinculada à empresa só quando a linha é emitida. | obrigatório |
| R12 | Tomador novo sem CEP, com CEP que a consulta não encontra ou sem conexão fica com o erro "endereço sem município: confira o CEP". A linha não fica pronta. | obrigatório |
| R13 | CNPJ de tomador sem cadastro segue o comportamento atual da planilha (cadastrar direto da linha). | obrigatório |
| R14 | Cada linha passa por `Emissor::validar` como as da planilha. Os erros aparecem na linha, e só linhas sem erro podem ser marcadas e emitidas. | obrigatório |
| R15 | Reimportar o mesmo e-mail (arquivo ou texto) não repete nota já autorizada ou em andamento; usa a chave de conteúdo da linha, como a planilha. | obrigatório |
| R16 | E-mail em que nenhum dos dois leiautes acha um pedido mostra "nenhum pedido de nota encontrado neste e-mail" e não cria linhas. | obrigatório |
| R17 | O corpo do e-mail não é gravado no banco, nem em arquivo, nem em log. Só os campos extraídos entram no pedido; a origem da nota registra o nome do arquivo (ou "texto colado") e o número da linha. | obrigatório |
| R18 | **Ler de novo** relê do disco o `.eml`/`.msg` escolhido (como na planilha) e, no texto colado, lê de novo o conteúdo atual do campo. | desejável |

## Critérios de aceite
Cada critério vira pelo menos um teste, com e-mails **fictícios** no leiaute dos reais.

- **CA1 (R1, R3)** — Dado um `.eml` com corpo `text/plain` em quoted-printable ISO-8859-1, quando ele é lido, então o texto sai com os acentos corretos.
- **CA2 (R3)** — Dado um `.eml` só com `text/html` em base64, quando ele é lido, então o texto sai sem as marcações HTML e com as quebras de linha dos parágrafos.
- **CA3 (R1, R4)** — Dado um `.msg` com o corpo em Unicode, quando ele é lido, então o texto é igual ao corpo original.
- **CA4 (R4)** — Dado um `.msg` com o corpo só em RTF, quando ele é lido, então a tela mostra "não consegui ler o corpo deste e-mail; cole o texto" e nenhuma linha.
- **CA5 (R5)** — Dado um texto com três blocos PACIENTE, em que um bloco tem CPF e VALOR na mesma linha, outro tem a cidade quebrada em duas linhas e o terceiro não tem "colocar no corpo da nota", quando ele é lido, então saem três linhas com os campos certos, e a terceira sem descrição própria.
- **CA6 (R5)** — Dado um bloco com rótulos em maiúsculas e minúsculas, acentos diferentes e espaços sobrando ("Cep :", "CIDADE/UF:"), quando ele é lido, então os campos são reconhecidos.
- **CA7 (R6, R7)** — Dado um texto corrido com o CNPJ da empresa prestadora, o CNPJ válido de uma filial do tomador, "R$ 1.234,56" e "07/2026", quando ele é lido com a empresa prestadora cadastrada, então sai uma linha com o tomador da filial, valor 1234,56 e mês 2026-07.
- **CA8 (R6)** — Dado um texto corrido com um número de 14 dígitos de dígito verificador inválido antes do CNPJ válido, quando ele é lido, então o tomador é o CNPJ válido.
- **CA9 (R7)** — Dado um e-mail sem período, quando ele é lido com o mês 2026-10 na tela, então a linha usa 2026-10.
- **CA10 (R8)** — Dado um e-mail lido sem serviço escolhido, então todas as linhas têm o erro "escolha o serviço" e nenhuma pode ser marcada.
- **CA11 (R9)** — Dado um bloco com "colocar no corpo da nota: Sessão de {mes}", quando a linha é montada para 2026-07, então a descrição traz o mês aplicado; dado um bloco sem esse rótulo, então a descrição é a padrão do serviço, com `{pix}` trocada pela chave da empresa.
- **CA12 (R10)** — Dado um paciente com CPF já cadastrado com outro endereço, quando o e-mail é lido e a linha é emitida, então a nota usa o endereço gravado, o cadastro não muda e a linha mostrou o aviso de dados diferentes.
- **CA13 (R11)** — Dado um paciente com CPF sem cadastro e CEP válido (consulta simulada), quando a linha é emitida, então a pessoa é cadastrada com o nome, o endereço e o código IBGE do município e vinculada à empresa. Antes de emitir, o banco não tem a pessoa.
- **CA14 (R12)** — Dado um paciente novo sem CEP, ou com CEP que a consulta (simulada) não encontra ou que falha por falta de conexão, quando o e-mail é lido, então a linha tem o erro "endereço sem município: confira o CEP".
- **CA15 (R14)** — Dado um bloco com valor ausente ou com mais de duas casas decimais, quando é lido, então a linha tem o mesmo erro que a planilha daria.
- **CA16 (R15)** — Dado um e-mail cuja linha já foi emitida e autorizada, quando ele é importado de novo, então a linha aparece como já emitida e não pode ser emitida de novo.
- **CA17 (R16)** — Dado um e-mail sem nenhum dos dois leiautes (por exemplo, só "bom dia, segue em anexo"), quando é lido, então a tela mostra "nenhum pedido de nota encontrado neste e-mail".
- **CA18 (R17)** — Dada a emissão de uma linha vinda de e-mail, então a tabela de origem guarda só o nome do arquivo e a linha, e nenhum registro de log contém o corpo do e-mail.

## Casos de borda e erros
- **Entrada vazia ou inválida:**
  - texto colado vazio: o botão não lê;
  - `.msg` ou `.eml` corrompido: "não consegui abrir o e-mail", sem linhas;
  - arquivo com outra extensão: segue a regra atual da planilha.
- **Texto citado (respostas e encaminhamentos):** a extração usa o corpo inteiro. Se os blocos se repetem no histórico citado, a repetição é marcada como linha repetida, como na planilha.
- **CPF com ou sem pontuação, valor com "R$", ponto de milhar ou vírgula:** o mesmo entendimento das funções da planilha (`documentos_da_planilha`, `valor_da_planilha`).
- **Vários "R$" ou vários períodos no texto corrido:** vale o primeiro; o operador confere na linha.
- **Falha da consulta de CEP:** a linha fica com erro (R12) e a leitura das outras linhas continua.
- **Repetição:** reimportar é idempotente pela chave de conteúdo (R15).

## Invariantes (o que não pode mudar)
- A importação de planilhas (colunas, sinônimos, chave de conteúdo, comportamento das linhas) continua igual.
- A DPS é validada antes de reservar número; a DPS é gravada como `enviando` antes do envio; em resposta incerta, consulta antes de reenviar ([fiscal.md](../../fiscal.md)).
- A validação do pedido continua só em `Emissor::validar`, e as regras fiscais ficam no backend.
- Nada é emitido sem o operador marcar a linha; não há emissão automática.
- O cadastro de um tomador existente não é alterado pela importação por e-mail.

## Restrições
- **Privacidade (LGPD):** os e-mails trazem CPF, endereço e dados de saúde.
  - O corpo do e-mail não é guardado nem registrado em log (R17).
  - Exemplos reais não entram no repositório: testes e documentação usam dados fictícios.
  - Nenhum serviço externo recebe o texto. A consulta de CEP envia só o CEP.
- **Sem dependência pesada:** o `.msg` é lido só nos campos necessários. Uma dependência nova (leitor CFB, parser MIME) é justificada no plano.
- **Desempenho:** um e-mail com até 100 blocos é lido em menos de 2 s, sem contar as consultas de CEP, que são feitas uma por vez, com o andamento na tela.
- **Compatibilidade:** sem migração de banco, se a origem atual da planilha (`Origem::Planilha` com o nome do arquivo) bastar. O plano confirma.

## Questões em aberto
- [ ] O leiaute de texto corrido deve aceitar também CPF como tomador, ou só CNPJ? (R6 aceita os dois; confirmar com os e-mails reais.)
- [ ] Rótulos além dos vistos nos exemplos (por exemplo "NOME", "PACIENTE/TITULAR", "TELEFONE", "E-MAIL") devem ser reconhecidos já na primeira versão?

## Histórico de mudanças
| Data | Mudança | Motivo |
|---|---|---|
| 2026-11-12 | Primeira versão | Entrevista do `/spec` |
