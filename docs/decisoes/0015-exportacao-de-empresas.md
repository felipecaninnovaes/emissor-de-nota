# 0015 — Exportação de empresas entre máquinas: só cadastros

Estado: aceita (18/10/2026).

**Contexto.** Cada máquina tem o próprio banco ([ADR 0002](0002-sqlite-por-maquina.md)) e reserva a própria numeração por empresa, ambiente e série ([ADR 0003](0003-serie-propria-e-numeracao.md), [ADR 0006](0006-numeracao-por-ambiente.md)). Levar uma empresa para outra máquina pelo backup `.nfbk` exige trocar o banco inteiro e usar a mesma senha mestra. A issue #1 pede exportar e importar só os cadastros de algumas empresas.

**Decisão.** O arquivo de exportação (`.nfemp`) leva só cadastros, sem o estado da numeração:
- **Conteúdo:** os dados da empresa e, de cada uma, os serviços ativos, os tomadores vinculados (com e-mails e retenção do vínculo) e as recorrências ativas.
- **O que fica de fora:** notas, DPS, XMLs, eventos, numeração, certificado A1 e configurações da máquina, como o SMTP e as pastas. Na máquina de destino, o operador importa o `.pfx` de novo.
- **Cifragem:** o arquivo inteiro vai cifrado com uma senha própria da exportação, com Argon2id e AES-256-GCM (os mesmos parâmetros e regras de senha do cofre), nunca com a senha mestra. Mesmo sem certificado, ele leva dados de tomadores (CPF, e-mails).
- **Empresa nova no destino:** é cadastrada com os dados do arquivo e marcada para conferir a numeração. A reserva de número fica bloqueada até rodar "Consultar na Sefin" (Empresas → Numeração / IM), que procura o primeiro número livre da série e tira a marca.
- **Empresa que já existe no destino** (mesmo CNPJ): atualiza nome, IM e quando informá-la na DPS, município, regime, telefone e e-mail. Ambiente, série, numeração, certificados e notas ficam como estão. Empresa arquivada volta ativa com o histórico, como no recadastro ([ADR 0010](0010-exclusao-de-empresa.md)).
- **Tomadores:** são vinculados pela pessoa com o mesmo documento, sem duplicar; os dados da pessoa, os e-mails e a retenção do vínculo são atualizados.
- **Serviços:** um serviço ativo com o mesmo nome é atualizado, como na importação de serviços.
- **Recorrências:** uma recorrência ativa com o mesmo tomador, serviço e descrição é atualizada; as demais são criadas. Assim, importar o mesmo arquivo duas vezes não duplica nada.

**Por quê.** Transferir a empresa (bloquear a origem) exigiria estado novo e um jeito de desfazer. Uma série por máquina exigiria que o operador entendesse séries. Exportar só cadastros não cria vínculo entre as máquinas. A colisão de números, que a Sefin recusa com E0014, fica coberta pela consulta obrigatória antes da primeira emissão. Quem precisa do histórico continua usando o `.nfbk`.

**Detalhes.**
- A marca é a coluna `empresa.conferir_numeracao` (migração 0008). Ajustar o número à mão não tira a marca: só a consulta à Sefin confere os números usados pela outra máquina.
- Se as duas máquinas continuarem emitindo pela mesma empresa e série, a consulta só vale no momento em que roda. Para emitir nas duas, cada máquina precisa usar uma série própria.
- Um arquivo inválido ou com a senha errada não grava nada. A importação é tudo ou nada: todas as empresas marcadas entram numa transação só, e um item inválido (recorrência de serviço ou tomador que não veio no arquivo, código de serviço inválido) desfaz tudo.
- Uma recorrência de serviço desativado não é exportada, porque o serviço não vai no arquivo.
