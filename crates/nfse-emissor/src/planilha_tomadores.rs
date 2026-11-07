//! Planilha de tomadores: importa o cadastro de clientes de CSV/XLSX/XLS/ODS, para migrar de
//! outro sistema sem digitar um por um.
//!
//! Uma linha por tomador; a primeira é o cabeçalho, comparado como nas outras planilhas (sem
//! acento, maiúsculas, espaços ou pontuação). Só CNPJ/CPF e Nome são obrigatórios. Célula vazia
//! mantém o que já está gravado (ver [`nfse_db::Banco::importar_tomadores`]).
//!
//! Uma lista só de CNPJs (ou de CNPJs e nomes) é aceita: a linha sem nome ou sem endereço fica
//! marcada para [`LinhaTomador::completar`] com a consulta de CNPJ, que o app faz linha a linha.

use std::collections::{BTreeMap, HashMap};

use nfse_core::modelo::{Documento, EnderecoNacional, Tomador};
use nfse_core::{Cnpj, Cpf};
use nfse_db::tomadores::{TipoTomador, TomadorImportado};
use serde::{Deserialize, Serialize};

use crate::planilha::{ler_tabela, normalizar};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
/// nome como vieram. Vai à tela e volta dela para ser completada (por isso as células).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinhaTomador {
    /// Número da linha na planilha (o cabeçalho é a 1).
    #[serde(rename = "linha")]
    pub numero: usize,
    pub documento: String,
    pub nome: String,
    pub item: Option<TomadorImportado>,
    pub erros: Vec<String>,
    /// Observações que não impedem a importação (de onde veio o dado, situação na Receita).
    #[serde(default)]
    pub avisos: Vec<String>,
    /// CNPJ sem nome ou sem endereço, à espera da consulta de CNPJ.
    #[serde(default)]
    pub a_completar: bool,
    /// Células preenchidas da linha, para validar de novo depois de completada.
    #[serde(default)]
    pub campos: BTreeMap<ColunaTomador, String>,
    /// Linha anterior com o mesmo CNPJ/CPF.
    #[serde(default)]
    pub repetida_de: Option<usize>,
}

/// Dados de fora da planilha (consulta de CNPJ ou cadastro já gravado) para completar uma linha.
/// Só entram nas células vazias; o endereço só entra inteiro e na linha que não tem nenhuma parte dele.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Complemento {
    pub nome: Option<String>,
    pub cep: Option<String>,
    pub municipio: Option<String>,
    pub logradouro: Option<String>,
    pub numero: Option<String>,
    pub complemento: Option<String>,
    pub bairro: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
}

const ENDERECO: [ColunaTomador; 6] = [
    C::Cep,
    C::Municipio,
    C::Logradouro,
    C::Numero,
    C::Complemento,
    C::Bairro,
];

impl LinhaTomador {
    /// CNPJ da linha que pede a consulta: válido, não repetido e sem o nome ou sem o endereço.
    pub fn cnpj_a_completar(&self) -> Option<Cnpj> {
        if self.repetida_de.is_some() {
            return None;
        }
        let Ok(Documento::Cnpj(cnpj)) = documento(self.campos.get(&C::Documento)?) else {
            return None;
        };
        let falta_nome = !self.campos.contains_key(&C::Nome);
        let falta_endereco = !ENDERECO.iter().any(|c| self.campos.contains_key(c));
        (falta_nome || falta_endereco).then_some(cnpj)
    }

    /// A linha validada de novo com as células vazias preenchidas por `dados`.
    pub fn completar(&self, dados: &Complemento) -> LinhaTomador {
        let mut campos = self.campos.clone();
        let valor = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        let mut vazio_recebe = |c: ColunaTomador, v: &Option<String>| {
            if let Some(v) = valor(v) {
                campos.entry(c).or_insert(v);
            }
        };
        vazio_recebe(C::Nome, &dados.nome);
        vazio_recebe(C::Telefone, &dados.telefone);
        vazio_recebe(C::Email, &dados.email);
        let sem_endereco = !ENDERECO.iter().any(|c| self.campos.contains_key(c));
        let completo = [&dados.cep, &dados.municipio, &dados.logradouro, &dados.bairro]
            .into_iter()
            .all(|v| valor(v).is_some());
        if sem_endereco && completo {
            for (c, v) in [
                (C::Cep, &dados.cep),
                (C::Municipio, &dados.municipio),
                (C::Logradouro, &dados.logradouro),
                (C::Numero, &dados.numero),
                (C::Complemento, &dados.complemento),
                (C::Bairro, &dados.bairro),
            ] {
                vazio_recebe(c, v);
            }
        }
        let mut nova = linha(self.numero, &campos);
        nova.avisos = self.avisos.clone();
        if let Some(primeira) = self.repetida_de {
            nova.marcar_repetida(primeira);
        }
        nova.a_completar = false;
        nova
    }

    fn marcar_repetida(&mut self, primeira: usize) {
        self.erros
            .push(format!("CNPJ/CPF repetido (já está na linha {primeira})"));
        self.item = None;
        self.repetida_de = Some(primeira);
        self.a_completar = false;
    }
}

/// Lê a planilha de tomadores.
pub fn ler(nome_arquivo: &str, bytes: &[u8]) -> Result<Vec<LinhaTomador>, String> {
    let mut linhas = ler_tabela(nome_arquivo, bytes)?.into_iter().enumerate();
    let (_, cabecalho) = linhas.next().ok_or("a planilha está vazia")?;
    let colunas: Vec<Option<ColunaTomador>> = cabecalho.iter().map(|c| ColunaTomador::do_cabecalho(c)).collect();
    // Sem a coluna Nome vale como lista de CNPJs: o nome vem da consulta.
    if !colunas.contains(&Some(C::Documento)) {
        return Err(format!(
            "a primeira linha precisa ter a coluna \"{}\" (baixe o modelo para ver os nomes aceitos)",
            C::Documento.titulo()
        ));
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
            Some(primeira) => l.marcar_repetida(*primeira),
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
    let endereco = if ENDERECO.iter().any(|c| texto(*c).is_some()) {
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
    let mut l = LinhaTomador {
        numero,
        documento: texto(C::Documento).unwrap_or_default().to_owned(),
        nome,
        item,
        erros,
        avisos: Vec::new(),
        a_completar: false,
        campos: campos.clone(),
        repetida_de: None,
    };
    l.a_completar = l.cnpj_a_completar().is_some();
    l
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

    fn da_receita() -> Complemento {
        Complemento {
            nome: Some("EMPRESA DA RECEITA LTDA".into()),
            cep: Some("15800000".into()),
            municipio: Some("3511102".into()),
            logradouro: Some("RUA EXEMPLO".into()),
            numero: Some("100".into()),
            complemento: None,
            bairro: Some("CENTRO".into()),
            telefone: Some("1735220000".into()),
            email: Some("contato@exemplo.com".into()),
        }
    }

    #[test]
    fn lista_so_de_cnpj_e_completada() {
        let csv = "CNPJ\r\n11222333000181\r\n52998224725\r\n11.222.333/0001-81\r\n";
        let linhas = ler("t.csv", csv.as_bytes()).unwrap();
        let [cnpj, cpf, repetida] = &linhas[..] else {
            panic!("{linhas:?}")
        };
        assert!(cnpj.a_completar && cnpj.item.is_none());
        assert!(cnpj.erros.iter().any(|e| e.contains("nome vazio")), "{:?}", cnpj.erros);
        assert!(!cpf.a_completar, "CPF não tem consulta");
        assert!(!repetida.a_completar, "repetida não consulta de novo");

        let completa = cnpj.completar(&da_receita());
        assert!(completa.erros.is_empty(), "{:?}", completa.erros);
        assert!(!completa.a_completar);
        let t = &completa.item.as_ref().unwrap().tomador;
        assert_eq!(t.nome, "EMPRESA DA RECEITA LTDA");
        assert_eq!(t.endereco.as_ref().unwrap().municipio, "3511102");
        assert_eq!(t.email.as_deref(), Some("contato@exemplo.com"));
    }

    #[test]
    fn completar_nao_troca_o_que_veio_na_planilha() {
        let csv = "CNPJ;Nome;Telefone\r\n11222333000181;MEU NOME;17999990000\r\n\
                   11222333000262;OUTRA;\r\n";
        let linhas = ler("t.csv", csv.as_bytes()).unwrap();
        assert!(linhas[0].a_completar, "com nome, mas sem endereço");
        assert!(linhas[0].item.is_some(), "importável mesmo sem consultar");
        let c = linhas[0].completar(&da_receita());
        let t = &c.item.as_ref().unwrap().tomador;
        assert_eq!(
            (t.nome.as_str(), t.telefone.as_deref()),
            ("MEU NOME", Some("17999990000"))
        );
        assert!(t.endereco.is_some());

        // Endereço incompleto na consulta não entra (a linha segue sem endereço).
        let sem_bairro = Complemento {
            bairro: None,
            ..da_receita()
        };
        let c = linhas[1].completar(&sem_bairro);
        assert!(c.erros.is_empty(), "{:?}", c.erros);
        assert!(c.item.unwrap().tomador.endereco.is_none());

        // Com parte do endereço na planilha, o da consulta não se mistura.
        let parcial = ler("t.csv", "CNPJ;CEP\r\n11222333000181;15800000\r\n".as_bytes()).unwrap();
        let c = parcial[0].completar(&da_receita());
        assert!(c.erros.iter().any(|e| e.contains("incompleto")), "{:?}", c.erros);
    }
}
