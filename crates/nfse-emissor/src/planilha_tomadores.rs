//! Planilha de tomadores: importa o cadastro de clientes de CSV/XLSX/XLS/ODS, para migrar de
//! outro sistema sem digitar um por um.
//!
//! Uma linha por tomador; a primeira é o cabeçalho, comparado como nas outras planilhas (sem
//! acento, maiúsculas, espaços ou pontuação). Só CNPJ/CPF e Nome são obrigatórios. Célula vazia
//! mantém o que já está gravado (ver [`nfse_db::Banco::importar_tomadores`]).

use std::collections::{BTreeMap, HashMap};

use nfse_core::modelo::{Documento, EnderecoNacional, Tomador};
use nfse_core::{Cnpj, Cpf};
use nfse_db::tomadores::{TipoTomador, TomadorImportado};

use crate::planilha::{ler_tabela, normalizar};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColunaTomador {
    Documento,
    Nome,
    InscricaoMunicipal,
    Cep,
    Municipio,
    Logradouro,
    Numero,
    Complemento,
    Bairro,
    Telefone,
    Email,
    EmailsEnvio,
    Tipo,
    RetemFederais,
    RetemIss,
}

use ColunaTomador as C;

impl ColunaTomador {
    pub const TODAS: [ColunaTomador; 15] = [
        C::Documento,
        C::Nome,
        C::InscricaoMunicipal,
        C::Cep,
        C::Municipio,
        C::Logradouro,
        C::Numero,
        C::Complemento,
        C::Bairro,
        C::Telefone,
        C::Email,
        C::EmailsEnvio,
        C::Tipo,
        C::RetemFederais,
        C::RetemIss,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            C::Documento => "CNPJ/CPF",
            C::Nome => "Nome",
            C::InscricaoMunicipal => "Inscricao municipal",
            C::Cep => "CEP",
            C::Municipio => "Municipio IBGE",
            C::Logradouro => "Logradouro",
            C::Numero => "Numero",
            C::Complemento => "Complemento",
            C::Bairro => "Bairro",
            C::Telefone => "Telefone",
            C::Email => "E-mail",
            C::EmailsEnvio => "E-mails de envio",
            C::Tipo => "Tipo de tomador",
            C::RetemFederais => "Retem federais",
            C::RetemIss => "Retem ISS",
        }
    }

    fn do_cabecalho(nome: &str) -> Option<ColunaTomador> {
        let n = normalizar(nome);
        if let Some(c) = Self::TODAS.into_iter().find(|c| normalizar(c.titulo()) == n) {
            return Some(c);
        }
        let c = match n.as_str() {
            "cnpj" | "cpf" | "documento" | "cpfcnpj" | "cnpjcpf" | "doc" => C::Documento,
            "razaosocial" | "nomerazaosocial" | "tomador" | "cliente" | "nomedotomador" => C::Nome,
            "im" | "inscricaomunicipaldotomador" => C::InscricaoMunicipal,
            "municipio" | "codigomunicipio" | "ibge" | "codigoibge" | "cmun" => C::Municipio,
            "endereco" | "rua" => C::Logradouro,
            "nro" | "num" => C::Numero,
            "fone" | "celular" => C::Telefone,
            "email" => C::Email,
            "emailsdeenvio" | "emailsenvio" | "emailenvio" | "emailsnota" | "emails" => C::EmailsEnvio,
            "tipo" | "tipotomador" => C::Tipo,
            "retemimpostosfederais" | "retencaofederal" | "retemfederal" => C::RetemFederais,
            "retemiss" | "retencaoiss" | "issretido" => C::RetemIss,
            _ => return None,
        };
        Some(c)
    }
}

/// Tomador lido de uma linha. Com `erros`, `item` fica vazio e a tela mostra o documento e o
/// nome como vieram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaTomador {
    /// Número da linha na planilha (o cabeçalho é a 1).
    pub numero: usize,
    pub documento: String,
    pub nome: String,
    pub item: Option<TomadorImportado>,
    pub erros: Vec<String>,
}

/// Lê a planilha de tomadores.
pub fn ler(nome_arquivo: &str, bytes: &[u8]) -> Result<Vec<LinhaTomador>, String> {
    let mut linhas = ler_tabela(nome_arquivo, bytes)?.into_iter().enumerate();
    let (_, cabecalho) = linhas.next().ok_or("a planilha está vazia")?;
    let colunas: Vec<Option<ColunaTomador>> = cabecalho.iter().map(|c| ColunaTomador::do_cabecalho(c)).collect();
    for obrigatoria in [C::Documento, C::Nome] {
        if !colunas.contains(&Some(obrigatoria)) {
            return Err(format!(
                "a primeira linha precisa ter a coluna \"{}\" (baixe o modelo para ver os nomes aceitos)",
                obrigatoria.titulo()
            ));
        }
    }
    let mut lidas: Vec<LinhaTomador> = linhas
        .filter_map(|(i, celulas)| {
            let campos: BTreeMap<ColunaTomador, String> = colunas
                .iter()
                .zip(celulas)
                .filter_map(|(coluna, valor)| Some(((*coluna)?, valor.trim().to_owned())))
                .filter(|(_, v)| !v.is_empty())
                .collect();
            (!campos.is_empty()).then(|| linha(i + 1, &campos))
        })
        .collect();
    let mut vistos: HashMap<String, usize> = HashMap::new();
    for l in &mut lidas {
        let Ok(d) = documento(&l.documento) else { continue };
        let chave = match d {
            Documento::Cnpj(c) => c.as_str().to_owned(),
            Documento::Cpf(c) => c.as_str().to_owned(),
        };
        match vistos.get(&chave) {
            Some(primeira) => {
                l.erros.push(format!("CNPJ/CPF repetido (já está na linha {primeira})"));
                l.item = None;
            }
            None => {
                vistos.insert(chave, l.numero);
            }
        }
    }
    Ok(lidas)
}

fn linha(numero: usize, campos: &BTreeMap<ColunaTomador, String>) -> LinhaTomador {
    let texto = |c: ColunaTomador| campos.get(&c).map(String::as_str);
    let mut erros = Vec::new();

    let documento = match texto(C::Documento).map(documento) {
        Some(Ok(d)) => Some(d),
        Some(Err(e)) => {
            erros.push(e);
            None
        }
        None => {
            erros.push("CNPJ/CPF vazio".into());
            None
        }
    };
    let nome = texto(C::Nome).unwrap_or_default().to_owned();
    if nome.is_empty() {
        erros.push("nome vazio".into());
    } else if nome.chars().count() > 300 {
        erros.push("nome com mais de 300 caracteres".into());
    }
    let inscricao_municipal = texto(C::InscricaoMunicipal).map(str::to_owned);
    if inscricao_municipal.as_ref().is_some_and(|im| im.chars().count() > 15) {
        erros.push("inscrição municipal com mais de 15 caracteres".into());
    }

    // Endereço: opcional, mas começado precisa de CEP, município, logradouro e bairro.
    let partes = [
        C::Cep,
        C::Municipio,
        C::Logradouro,
        C::Numero,
        C::Complemento,
        C::Bairro,
    ];
    let endereco = if partes.iter().any(|c| texto(*c).is_some()) {
        let faltando: Vec<&str> = [C::Cep, C::Municipio, C::Logradouro, C::Bairro]
            .into_iter()
            .filter(|c| texto(*c).is_none())
            .map(ColunaTomador::titulo)
            .collect();
        if !faltando.is_empty() {
            erros.push(format!("endereço incompleto: falta {}", faltando.join(", ")));
        }
        let cep = completar(texto(C::Cep), 8);
        if cep.as_ref().is_some_and(|c| c.len() != 8) {
            erros.push("CEP precisa ter 8 dígitos".into());
        }
        let municipio = digitos(texto(C::Municipio).unwrap_or_default());
        if texto(C::Municipio).is_some() && municipio.len() != 7 {
            erros.push("município: use o código IBGE de 7 dígitos (ex.: 3511102)".into());
        }
        Some(EnderecoNacional {
            municipio,
            cep: cep.unwrap_or_default(),
            logradouro: texto(C::Logradouro).unwrap_or_default().to_owned(),
            numero: texto(C::Numero).unwrap_or("S/N").to_owned(),
            complemento: texto(C::Complemento).map(str::to_owned),
            bairro: texto(C::Bairro).unwrap_or_default().to_owned(),
        })
    } else {
        None
    };

    let mut sim_nao = |c: ColunaTomador| -> Option<bool> {
        let t = texto(c)?;
        match normalizar(t).as_str() {
            "sim" | "s" | "x" | "1" | "true" | "verdadeiro" => Some(true),
            "nao" | "n" | "0" | "false" | "falso" => Some(false),
            _ => {
                erros.push(format!("{}: use Sim ou Não (veio \"{t}\")", c.titulo()));
                None
            }
        }
    };
    let retem_federais = sim_nao(C::RetemFederais);
    let retem_iss = sim_nao(C::RetemIss);
    let tipo = texto(C::Tipo).and_then(|t| match normalizar(t).as_str() {
        "privado" | "privada" | "empresaprivada" | "pessoafisica" | "pf" | "pj" => Some(TipoTomador::Privado),
        "federal" | "uniao" | "orgaodauniao" => Some(TipoTomador::Federal),
        "estadual" | "estado" => Some(TipoTomador::Estadual),
        "distritofederal" | "df" => Some(TipoTomador::DistritoFederal),
        "municipal" | "municipio" | "prefeitura" => Some(TipoTomador::Municipal),
        _ => {
            erros.push(format!(
                "tipo de tomador \"{t}\": use Privado, Federal, Estadual, Distrito Federal ou Municipal"
            ));
            None
        }
    });

    let item = documento
        .filter(|_| erros.is_empty())
        .map(|documento| TomadorImportado {
            tomador: Tomador {
                documento,
                inscricao_municipal,
                nome: nome.clone(),
                endereco,
                telefone: texto(C::Telefone).map(digitos).filter(|t| !t.is_empty()),
                email: texto(C::Email).map(str::to_owned),
            },
            emails: texto(C::EmailsEnvio).map(str::to_owned),
            retem_federais,
            retem_iss,
            tipo,
        });
    LinhaTomador {
        numero,
        documento: texto(C::Documento).unwrap_or_default().to_owned(),
        nome,
        item,
        erros,
    }
}

/// CNPJ (também alfanumérico) ou CPF, com ou sem pontuação. Número salvo como número na planilha
/// perde os zeros à esquerda; eles são repostos.
fn documento(texto: &str) -> Result<Documento, String> {
    let limpo: String = texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let so_digitos = limpo.chars().all(|c| c.is_ascii_digit());
    let invalido = |e: &dyn std::fmt::Display| format!("CNPJ/CPF {texto} inválido: {e}");
    if so_digitos && limpo.len() <= 11 {
        return Cpf::parse(&format!("{limpo:0>11}"))
            .map(Documento::Cpf)
            .map_err(|e| invalido(&e));
    }
    let cnpj = if so_digitos { format!("{limpo:0>14}") } else { limpo };
    Cnpj::parse(&cnpj).map(Documento::Cnpj).map_err(|e| invalido(&e))
}

fn digitos(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

/// Dígitos com os zeros à esquerda que a planilha tira de um número.
fn completar(texto: Option<&str>, n: usize) -> Option<String> {
    texto.map(|t| format!("{:0>n$}", digitos(t)))
}

/// Modelo com um tomador de exemplo e os nomes das colunas aceitas.
pub fn modelo_csv() -> String {
    let mut escritor = csv::WriterBuilder::new()
        .delimiter(b';')
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    let cabecalho: Vec<&str> = ColunaTomador::TODAS.iter().map(|c| c.titulo()).collect();
    escritor.write_record(&cabecalho).expect("escrita em memória");
    escritor
        .write_record([
            "11.222.333/0001-81",
            "CLIENTE EXEMPLO LTDA",
            "12345",
            "15800-000",
            "3511102",
            "Rua Exemplo",
            "100",
            "Sala 1",
            "Centro",
            "(17) 3522-0000",
            "contato@exemplo.com.br",
            "financeiro@exemplo.com.br; contador@exemplo.com.br",
            "Privado",
            "Não",
            "Não",
        ])
        .expect("escrita em memória");
    String::from_utf8(escritor.into_inner().expect("escrita em memória")).expect("CSV em UTF-8")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn modelo_e_lido_de_volta() {
        let linhas = ler("t.csv", modelo_csv().as_bytes()).unwrap();
        assert_eq!(linhas.len(), 1);
        let l = &linhas[0];
        assert!(l.erros.is_empty(), "{:?}", l.erros);
        let item = l.item.as_ref().unwrap();
        let t = &item.tomador;
        assert_eq!(t.documento, Documento::Cnpj(Cnpj::parse("11222333000181").unwrap()));
        assert_eq!(t.inscricao_municipal.as_deref(), Some("12345"));
        let e = t.endereco.as_ref().unwrap();
        assert_eq!((e.cep.as_str(), e.municipio.as_str()), ("15800000", "3511102"));
        assert_eq!(t.telefone.as_deref(), Some("1735220000"));
        assert_eq!(item.retem_federais, Some(false));
        assert_eq!(item.tipo, Some(TipoTomador::Privado));
    }

    #[test]
    fn aceita_o_minimo_e_repoe_zeros() {
        // CPF e CNPJ gravados como número (sem os zeros à esquerda), cabeçalhos de outro sistema.
        let csv = "Razão social;CPF/CNPJ;Retém ISS\r\nMARIA;1234567890;\r\nEMPRESA;1222333000128;sim\r\n";
        let linhas = ler("t.csv", csv.as_bytes()).unwrap();
        assert!(linhas.iter().all(|l| l.erros.is_empty()), "{linhas:?}");
        let maria = linhas[0].item.as_ref().unwrap();
        assert_eq!(
            maria.tomador.documento,
            Documento::Cpf(Cpf::parse("01234567890").unwrap())
        );
        assert_eq!(maria.retem_iss, None, "vazio mantém o que já está gravado");
        assert!(maria.tomador.endereco.is_none());
        let empresa = linhas[1].item.as_ref().unwrap();
        assert!(matches!(empresa.tomador.documento, Documento::Cnpj(_)));
        assert_eq!(empresa.retem_iss, Some(true));
    }

    #[test]
    fn aponta_erros_por_linha() {
        let csv = "CNPJ/CPF;Nome;Municipio;CEP;Tipo de tomador;Retem federais\r\n\
                   11222333000181;A;Catanduva;15800000;;\r\n\
                   11222333000181;B;;;prefeitura;talvez\r\n\
                   123;C;;;;\r\n";
        let linhas = ler("t.csv", csv.as_bytes()).unwrap();
        assert!(
            linhas[0].erros.iter().any(|e| e.contains("IBGE")),
            "{:?}",
            linhas[0].erros
        );
        assert!(
            linhas[0].erros.iter().any(|e| e.contains("incompleto")),
            "{:?}",
            linhas[0].erros
        );
        assert!(
            linhas[1].erros.iter().any(|e| e.contains("repetido")),
            "{:?}",
            linhas[1].erros
        );
        assert!(
            linhas[1].erros.iter().any(|e| e.contains("Sim ou Não")),
            "{:?}",
            linhas[1].erros
        );
        assert!(linhas.iter().all(|l| l.item.is_none()));
        assert_eq!(linhas[2].nome, "C");
        assert!(
            linhas[2].erros.iter().any(|e| e.contains("inválido")),
            "{:?}",
            linhas[2].erros
        );
    }

    #[test]
    fn sem_coluna_obrigatoria() {
        let erro = ler("t.csv", b"Nome;CEP\r\nX;15800000\r\n").unwrap_err();
        assert!(erro.contains("CNPJ/CPF"), "{erro}");
    }
}
