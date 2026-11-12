//! Extração dos pedidos de nota do texto de um e-mail.
//!
//! **Blocos com rótulos:** cada bloco começa no rótulo PACIENTE e traz CPF, ENDEREÇO, BAIRRO, CEP,
//! CIDADE (ou CIDADE/UF), VALOR e "colocar no corpo da nota". Os rótulos terminam em `:` e são
//! reconhecidos sem acento, sem diferença de maiúsculas e com qualquer espaço em volta; pode haver
//! vários na mesma linha. O valor de um rótulo vai até o próximo rótulo, uma linha em branco ou
//! uma linha que começa com outro rótulo qualquer (`TELEFONE:`, por exemplo), e pode continuar na
//! linha seguinte.
//!
//! **Texto corrido** (sem rótulo PACIENTE): um pedido só, com o primeiro CNPJ ou CPF de dígito
//! verificador válido que não seja de uma empresa cadastrada, o primeiro valor após "R$" e o
//! primeiro período. Sem documento de tomador, não há pedido.
//!
//! **Período:** o primeiro `MM/AAAA` do e-mail, que vale para todos os pedidos dele; o mês de uma
//! data completa (`DD/MM/AAAA`) não conta.
//!
//! Os campos saem como texto; a conversão (documento, valor, mês) é a da planilha, em
//! `importacao_email`.

use nfse_core::texto::sem_acento_minusculo;

/// Mensagem quando nenhum leiaute acha pedido.
pub const NENHUM_PEDIDO: &str = "nenhum pedido de nota encontrado neste e-mail";

/// Pedido de nota encontrado no e-mail, com os campos como estavam no texto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PedidoEmail {
    /// Número do pedido no e-mail (1, 2, …), usado como "linha" da importação.
    pub numero: usize,
    pub nome: Option<String>,
    /// CPF ou CNPJ do tomador.
    pub documento: Option<String>,
    pub endereco: Option<String>,
    pub bairro: Option<String>,
    pub cep: Option<String>,
    /// Cidade, às vezes com a UF ("Catanduva/SP").
    pub cidade: Option<String>,
    pub valor: Option<String>,
    /// Texto de "colocar no corpo da nota".
    pub descricao: Option<String>,
    /// Período (`MM/AAAA`) encontrado no e-mail.
    pub mes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rotulo {
    Paciente,
    Cpf,
    Endereco,
    Bairro,
    Cep,
    Cidade,
    Valor,
    Descricao,
    /// Rótulo fora da lista, no começo da linha: só encerra o valor anterior.
    Outro,
}

/// Rótulos reconhecidos, já sem acento e em minúsculas. Espaço casa com qualquer espaço (e
/// quebra de linha); `/` aceita espaços em volta.
const ROTULOS: [(&str, Rotulo); 9] = [
    ("colocar no corpo da nota", Rotulo::Descricao),
    ("paciente", Rotulo::Paciente),
    ("cpf", Rotulo::Cpf),
    ("endereco", Rotulo::Endereco),
    ("bairro", Rotulo::Bairro),
    ("cep", Rotulo::Cep),
    ("cidade/uf", Rotulo::Cidade),
    ("cidade", Rotulo::Cidade),
    ("valor", Rotulo::Valor),
];

/// Pedidos do e-mail; sem nenhum, [`NENHUM_PEDIDO`].
///
/// `cnpjs_das_empresas`: CNPJs (só os 14 caracteres) das empresas cadastradas, que no texto
/// corrido não são o tomador.
pub fn extrair(texto: &str, cnpjs_das_empresas: &[String]) -> Result<Vec<PedidoEmail>, String> {
    let mut pedidos = blocos(texto);
    if pedidos.is_empty() {
        pedidos.extend(texto_corrido(texto, cnpjs_das_empresas));
    }
    if pedidos.is_empty() {
        return Err(NENHUM_PEDIDO.into());
    }
    let mes = periodo(texto);
    for p in &mut pedidos {
        p.mes.clone_from(&mes);
    }
    Ok(pedidos)
}

/// Leiaute de texto corrido: um pedido, se houver documento de tomador.
fn texto_corrido(texto: &str, cnpjs_das_empresas: &[String]) -> Option<PedidoEmail> {
    let documento = documentos(texto).find(|d| !cnpjs_das_empresas.contains(d))?;
    Some(PedidoEmail {
        numero: 1,
        documento: Some(documento),
        valor: valor_apos_cifrao(texto),
        ..Default::default()
    })
}

/// CNPJs e CPFs de dígito verificador válido, na ordem do texto, sem pontuação.
fn documentos(texto: &str) -> impl Iterator<Item = String> + '_ {
    texto
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-')))
        .filter_map(|trecho| {
            let doc: String = trecho.chars().filter(char::is_ascii_alphanumeric).collect();
            match doc.len() {
                14 => nfse_core::Cnpj::parse(&doc).ok().map(|c| c.as_str().to_owned()),
                11 => nfse_core::Cpf::parse(&doc).ok().map(|c| c.as_str().to_owned()),
                _ => None,
            }
        })
}

/// O primeiro valor depois de "R$", só os dígitos, pontos e vírgulas ("1.234,56").
fn valor_apos_cifrao(texto: &str) -> Option<String> {
    let (_, depois) = texto.split_once("R$")?;
    let valor: String = depois
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
        .collect();
    let valor = valor.trim_end_matches(['.', ',']);
    (!valor.is_empty()).then(|| valor.to_owned())
}

/// O primeiro `MM/AAAA` (ou `M/AAAA`) que não seja parte de uma data, como `MM/AAAA`.
fn periodo(texto: &str) -> Option<String> {
    let c: Vec<char> = texto.chars().collect();
    let digito = |i: usize| c.get(i).is_some_and(char::is_ascii_digit);
    (0..c.len()).find_map(|i| {
        if c[i] != '/' || !(digito(i + 1) && digito(i + 2) && digito(i + 3) && digito(i + 4)) || digito(i + 5) {
            return None;
        }
        let inicio_mes = if i >= 2 && digito(i - 2) {
            i - 2
        } else {
            i.checked_sub(1)?
        };
        if !digito(i - 1) {
            return None;
        }
        // Antes do mês não pode haver dígito nem barra (seria o dia de uma data completa).
        if inicio_mes > 0 && (digito(inicio_mes - 1) || c[inicio_mes - 1] == '/') {
            return None;
        }
        let mes: u32 = c[inicio_mes..i].iter().collect::<String>().parse().ok()?;
        let ano: String = c[i + 1..i + 5].iter().collect();
        (1..=12).contains(&mes).then(|| format!("{mes:02}/{ano}"))
    })
}

/// Leiaute de blocos: um pedido por rótulo PACIENTE.
fn blocos(texto: &str) -> Vec<PedidoEmail> {
    let original: Vec<char> = texto.chars().collect();
    let normal = normalizar(&original);
    let achados = achar_rotulos(&normal);

    let mut pedidos: Vec<PedidoEmail> = Vec::new();
    for (k, &(rotulo, _, valor_inicio)) in achados.iter().enumerate() {
        let fim = achados.get(k + 1).map_or(original.len(), |&(_, inicio, _)| inicio);
        let valor = valor_do_rotulo(&original[valor_inicio..fim]);
        if rotulo == Rotulo::Paciente {
            pedidos.push(PedidoEmail {
                numero: pedidos.len() + 1,
                ..Default::default()
            });
        }
        let Some(pedido) = pedidos.last_mut() else {
            continue; // rótulos antes do primeiro PACIENTE
        };
        let campo = match rotulo {
            Rotulo::Paciente => &mut pedido.nome,
            Rotulo::Cpf => &mut pedido.documento,
            Rotulo::Endereco => &mut pedido.endereco,
            Rotulo::Bairro => &mut pedido.bairro,
            Rotulo::Cep => &mut pedido.cep,
            Rotulo::Cidade => &mut pedido.cidade,
            Rotulo::Valor => &mut pedido.valor,
            Rotulo::Descricao => &mut pedido.descricao,
            Rotulo::Outro => continue,
        };
        // Rótulo repetido no mesmo bloco: vale o primeiro.
        if campo.is_none() && !valor.is_empty() {
            *campo = Some(valor);
        }
    }
    pedidos
}

/// Um caractere minúsculo e sem acento por caractere do original (mesmas posições).
fn normalizar(original: &[char]) -> Vec<char> {
    original
        .iter()
        .map(|&c| {
            if c.is_ascii() {
                c.to_ascii_lowercase()
            } else {
                sem_acento_minusculo(c.encode_utf8(&mut [0; 4]))
                    .chars()
                    .next()
                    .unwrap_or(c)
            }
        })
        .collect()
}

/// Rótulos achados, em ordem: (rótulo, início do rótulo, início do valor).
fn achar_rotulos(normal: &[char]) -> Vec<(Rotulo, usize, usize)> {
    let mut achados = Vec::new();
    let mut i = 0;
    while i < normal.len() {
        let inicio_de_palavra = i == 0 || !normal[i - 1].is_alphanumeric();
        let achado = inicio_de_palavra
            .then(|| {
                ROTULOS
                    .iter()
                    .find_map(|&(padrao, rotulo)| casar_rotulo(normal, i, padrao).map(|fim| (rotulo, fim)))
            })
            .flatten()
            .or_else(|| rotulo_qualquer(normal, i).map(|fim| (Rotulo::Outro, fim)));
        match achado {
            Some((rotulo, valor_inicio)) => {
                achados.push((rotulo, i, valor_inicio));
                i = valor_inicio;
            }
            None => i += 1,
        }
    }
    achados
}

/// Se `padrao` seguido de `:` começa em `i`, a posição logo depois do `:`.
fn casar_rotulo(normal: &[char], i: usize, padrao: &str) -> Option<usize> {
    let mut j = i;
    for p in padrao.chars() {
        match p {
            ' ' => {
                let antes = j;
                j = pular_espacos(normal, j);
                if j == antes {
                    return None;
                }
            }
            '/' => {
                j = pular_espacos(normal, j);
                (normal.get(j) == Some(&'/')).then_some(())?;
                j = pular_espacos(normal, j + 1);
            }
            _ => {
                (normal.get(j) == Some(&p)).then_some(())?;
                j += 1;
            }
        }
    }
    // "cidade" não pode casar o começo de "cidades"; "cpf" não casa "cpfs".
    if normal.get(j).is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    j = pular_espacos_na_linha(normal, j);
    (normal.get(j) == Some(&':')).then_some(j + 1)
}

/// Rótulo fora da lista no começo de uma linha: até 30 letras, espaços ou `/` antes de `:`.
fn rotulo_qualquer(normal: &[char], i: usize) -> Option<usize> {
    let comeco_de_linha = normal[..i]
        .iter()
        .rev()
        .take_while(|&&c| c != '\n')
        .all(|c| c.is_whitespace());
    if !comeco_de_linha || !normal[i].is_alphabetic() {
        return None;
    }
    let fim = normal[i..]
        .iter()
        .take(31)
        .position(|&c| !(c.is_alphabetic() || c == ' ' || c == '/' || c == '\t'))?;
    (normal.get(i + fim) == Some(&':')).then_some(i + fim + 1)
}

fn pular_espacos(normal: &[char], mut j: usize) -> usize {
    while normal.get(j).is_some_and(|c| c.is_whitespace()) {
        j += 1;
    }
    j
}

fn pular_espacos_na_linha(normal: &[char], mut j: usize) -> usize {
    while normal.get(j).is_some_and(|&c| c == ' ' || c == '\t') {
        j += 1;
    }
    j
}

/// Valor até a primeira linha em branco, com espaços e quebras de linha reduzidos a um espaço.
fn valor_do_rotulo(trecho: &[char]) -> String {
    let texto: String = trecho.iter().collect();
    let mut linhas = Vec::new();
    for (k, linha) in texto.lines().enumerate() {
        if linha.trim().is_empty() && k > 0 {
            break;
        }
        linhas.push(linha);
    }
    linhas.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    const TRES_BLOCOS: &str = "Bom dia, seguem os pedidos de outubro:

PACIENTE: Ana Beatriz Souza
CPF: 529.982.247-25      VALOR: R$ 150,00
ENDEREÇO: Rua das Flores, 100   BAIRRO: Centro
CEP: 15800-000   CIDADE: Catanduva/SP
colocar no corpo da nota: Sessões de fisioterapia em {mes_ref}

PACIENTE: Bruno Lima
CPF: 111.444.777-35
ENDEREÇO: Av. Brasil, 2000
BAIRRO: Jardim
CEP: 15015-000
CIDADE: São José do Rio
Preto/SP
VALOR: R$ 1.200,00
colocar no corpo da nota: Consulta

PACIENTE: Carla Dias
CPF: 39053344705
VALOR: 80,00
CEP: 01001-000 CIDADE: São Paulo/SP

Atenciosamente,
Fulana";

    #[test]
    fn tres_blocos_irregulares() {
        let pedidos = extrair(TRES_BLOCOS, &[]).unwrap();
        assert_eq!(pedidos.len(), 3);
        assert_eq!(
            pedidos[0],
            PedidoEmail {
                numero: 1,
                nome: s("Ana Beatriz Souza"),
                documento: s("529.982.247-25"),
                endereco: s("Rua das Flores, 100"),
                bairro: s("Centro"),
                cep: s("15800-000"),
                cidade: s("Catanduva/SP"),
                valor: s("R$ 150,00"),
                descricao: s("Sessões de fisioterapia em {mes_ref}"),
                mes: None,
            }
        );
        assert_eq!(pedidos[1].numero, 2);
        assert_eq!(pedidos[1].cidade, s("São José do Rio Preto/SP"));
        assert_eq!(pedidos[1].valor, s("R$ 1.200,00"));
        assert_eq!(pedidos[1].descricao, s("Consulta"));
        assert_eq!(
            pedidos[2],
            PedidoEmail {
                numero: 3,
                nome: s("Carla Dias"),
                documento: s("39053344705"),
                cep: s("01001-000"),
                cidade: s("São Paulo/SP"),
                valor: s("80,00"),
                ..Default::default()
            }
        );
    }

    #[test]
    fn rotulos_sem_acento_e_espacos() {
        let texto = "  Paciente :   Daniel    Rocha\n\
                     cpf:529.982.247-25\n\
                     Endereco :Rua Um, 1\n\
                     bairro\t:  Vila Nova\n\
                     Cep : 15800-000\n\
                     CIDADE/UF: Catanduva - SP\n\
                     Valor : R$ 99,90\n\
                     Colocar no  Corpo da Nota : Sessão única";
        let p = &extrair(texto, &[]).unwrap()[0];
        assert_eq!(p.nome, s("Daniel Rocha"));
        assert_eq!(p.documento, s("529.982.247-25"));
        assert_eq!(p.endereco, s("Rua Um, 1"));
        assert_eq!(p.bairro, s("Vila Nova"));
        assert_eq!(p.cep, s("15800-000"));
        assert_eq!(p.cidade, s("Catanduva - SP"));
        assert_eq!(p.valor, s("R$ 99,90"));
        assert_eq!(p.descricao, s("Sessão única"));

        let p = &extrair("PACIENTE: Eva\ncidade / uf : Tabapuã/SP", &[]).unwrap()[0];
        assert_eq!(p.cidade, s("Tabapuã/SP"));
    }

    #[test]
    fn rotulo_desconhecido_encerra_o_valor() {
        let texto = "PACIENTE: Eva Marques\nTELEFONE: (17) 99999-0000\nVALOR: R$ 10,00";
        let p = &extrair(texto, &[]).unwrap()[0];
        assert_eq!(p.nome, s("Eva Marques"));
        assert_eq!(p.valor, s("R$ 10,00"));
    }

    #[test]
    fn palavra_dentro_do_texto_nao_e_rotulo() {
        // "valor" sem `:` e "cidades" não são rótulos.
        let texto =
            "PACIENTE: Eva\nCPF: 529.982.247-25\ncolocar no corpo da nota: valor das sessões nas cidades: A e B";
        let p = &extrair(texto, &[]).unwrap()[0];
        assert_eq!(p.valor, None);
        assert_eq!(p.descricao, s("valor das sessões nas cidades: A e B"));
    }

    #[test]
    fn sem_paciente_vale_o_texto_corrido() {
        // Sem PACIENTE, os rótulos não contam: o valor do texto corrido é só o que vem após "R$".
        let pedidos = extrair("CPF: 529.982.247-25\nVALOR: 10,00", &[]).unwrap();
        assert_eq!(
            pedidos,
            [PedidoEmail {
                numero: 1,
                documento: s("52998224725"),
                ..Default::default()
            }]
        );
    }

    const PRESTADORA: &str = "11222333000181";

    fn corrido(texto: &str) -> Result<Vec<PedidoEmail>, String> {
        extrair(texto, &[PRESTADORA.to_string()])
    }

    #[test]
    fn texto_corrido_ignora_cnpj_da_prestadora() {
        let texto = "Prezados, solicitamos a nota de serviços da Clínica Fictícia \
                     (CNPJ 11.222.333/0001-81) para a nossa filial CNPJ 11.444.777/0002-42, \
                     no valor de R$ 1.234,56, referente aos atendimentos de 07/2026.\n\
                     Enviado em 10/11/2026. Outro valor citado: R$ 9,99 em 08/2026.";
        let pedidos = corrido(texto).unwrap();
        assert_eq!(
            pedidos,
            [PedidoEmail {
                numero: 1,
                documento: s("11444777000242"),
                valor: s("1.234,56"),
                mes: s("07/2026"),
                ..Default::default()
            }]
        );
    }

    #[test]
    fn texto_corrido_pula_dv_invalido() {
        let texto = "Protocolo 12345678901234. Tomador: 11.444.777/0001-61. Valor R$80,00.";
        let p = &corrido(texto).unwrap()[0];
        assert_eq!(p.documento, s("11444777000161"));
        assert_eq!(p.valor, s("80,00"));
        assert_eq!(p.mes, None);
    }

    #[test]
    fn texto_corrido_aceita_cpf() {
        let p = &corrido("Nota para o CPF 529.982.247-25 de R$ 150,00 (7/2026).").unwrap()[0];
        assert_eq!(p.documento, s("52998224725"));
        assert_eq!(p.mes, s("07/2026"));
    }

    #[test]
    fn data_completa_nao_e_periodo() {
        let p = &corrido("CNPJ 11444777000161, R$ 10,00, enviado em 15/09/2026.").unwrap()[0];
        assert_eq!(p.mes, None);
    }

    #[test]
    fn blocos_recebem_o_periodo_do_email() {
        let texto = "Pedidos de 10/2026 (enviado em 03/11/2026):\n\nPACIENTE: Eva\nVALOR: 10,00\n\nPACIENTE: Ivo";
        let pedidos = corrido(texto).unwrap();
        assert!(pedidos.iter().all(|p| p.mes == s("10/2026")), "{pedidos:?}");
    }

    #[test]
    fn sem_pedido() {
        assert_eq!(corrido("Bom dia, segue em anexo.").unwrap_err(), NENHUM_PEDIDO);
        // Só o CNPJ da prestadora (na assinatura, por exemplo) não é pedido.
        assert_eq!(
            corrido("Obrigado. Clínica Fictícia - CNPJ 11.222.333/0001-81 - R$ 10,00").unwrap_err(),
            NENHUM_PEDIDO
        );
    }

    #[test]
    fn cem_blocos_em_menos_de_2s() {
        let bloco = "PACIENTE: Fulano de Tal\nCPF: 529.982.247-25   VALOR: R$ 150,00\n\
                     ENDEREÇO: Rua das Flores, 100   BAIRRO: Centro\n\
                     CEP: 15800-000   CIDADE: Catanduva/SP\n\
                     colocar no corpo da nota: Sessões de fisioterapia\n\n";
        let texto = bloco.repeat(100);
        let inicio = std::time::Instant::now();
        let pedidos = extrair(&texto, &[]).unwrap();
        assert_eq!(pedidos.len(), 100);
        assert!(inicio.elapsed() < std::time::Duration::from_secs(2));
    }
}
