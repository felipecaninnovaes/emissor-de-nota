//! Leitura da planilha de importação (CSV, XLSX, XLS ou ODS) em linhas de texto por coluna.
//!
//! A primeira linha é o cabeçalho; os nomes são comparados sem acento, maiúsculas, espaços ou
//! pontuação, e aceitam sinônimos (ex.: "CNPJ do tomador", "Convênio"). Colunas desconhecidas são
//! ignoradas. O CSV pode vir separado por `;` ou `,`, em UTF-8 ou Windows-1252 (Excel brasileiro).

use std::collections::BTreeMap;
use std::io::Cursor;

use calamine::{Data, Reader};
use nfse_core::texto::sem_acento_minusculo;

/// Colunas reconhecidas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Coluna {
    /// CNPJ do prestador (se faltar, usa a empresa escolhida na tela).
    Empresa,
    /// CNPJ/CPF do tomador.
    Tomador,
    Valor,
    /// Nome do serviço cadastrado ou código de tributação nacional (6 dígitos).
    Servico,
    Descricao,
    /// Mês de referência (`AAAA-MM`, `MM/AAAA` ou data).
    Mes,
    Informacoes,
}

impl Coluna {
    pub const TODAS: [Coluna; 7] = [
        Coluna::Empresa,
        Coluna::Tomador,
        Coluna::Valor,
        Coluna::Servico,
        Coluna::Descricao,
        Coluna::Mes,
        Coluna::Informacoes,
    ];

    /// Nome no modelo de planilha.
    pub fn titulo(self) -> &'static str {
        match self {
            Coluna::Empresa => "CNPJ empresa",
            Coluna::Tomador => "CNPJ/CPF tomador",
            Coluna::Valor => "Valor",
            Coluna::Servico => "Servico",
            Coluna::Descricao => "Descricao",
            Coluna::Mes => "Mes referencia",
            Coluna::Informacoes => "Informacoes complementares",
        }
    }

    fn do_cabecalho(nome: &str) -> Option<Coluna> {
        let n = normalizar(nome);
        let c = match n.as_str() {
            "empresa" | "cnpjempresa" | "prestador" | "cnpjprestador" | "cnpjdoprestador" | "cnpjdaempresa" => {
                Coluna::Empresa
            }
            "tomador" | "cnpjtomador" | "cpfcnpjtomador" | "cnpjcpftomador" | "cnpjdotomador" | "documentotomador"
            | "cnpj" | "cpfcnpj" | "cnpjcpf" | "documento" | "convenio" | "cnpjconvenio" => Coluna::Tomador,
            "valor" | "valorservico" | "valordoservico" | "valortotal" | "vservico" => Coluna::Valor,
            "servico" | "codigoservico" | "codigo" | "ctribnac" | "codigotributacaonacional" => Coluna::Servico,
            "descricao" | "discriminacao" | "descricaoservico" | "descricaodoservico" => Coluna::Descricao,
            "mes" | "mesreferencia" | "mesdereferencia" | "referencia" | "competencia" => Coluna::Mes,
            "informacoes"
            | "informacoescomplementares"
            | "infcomplementares"
            | "observacao"
            | "observacoes"
            | "obs" => Coluna::Informacoes,
            _ => return None,
        };
        Some(c)
    }
}

/// Linha de dados com o número que ela tem na planilha (o cabeçalho é a linha 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaPlanilha {
    pub numero: usize,
    pub campos: BTreeMap<Coluna, String>,
}

impl LinhaPlanilha {
    pub fn campo(&self, c: Coluna) -> Option<&str> {
        self.campos.get(&c).map(String::as_str).filter(|v| !v.is_empty())
    }
}

/// Sem acento, minúsculo e só letras e dígitos.
pub(crate) fn normalizar(texto: &str) -> String {
    sem_acento_minusculo(texto)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Lê a planilha pelo nome do arquivo (extensão) e conteúdo.
pub fn ler(nome_arquivo: &str, bytes: &[u8]) -> Result<Vec<LinhaPlanilha>, String> {
    montar_linhas(ler_tabela(nome_arquivo, bytes)?)
}

/// Células da primeira aba (ou do CSV) como texto, linha a linha, cabeçalho incluído.
pub(crate) fn ler_tabela(nome_arquivo: &str, bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let extensao = nome_arquivo
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let tabela = match extensao.as_str() {
        "csv" | "txt" => ler_csv(bytes)?,
        "xlsx" | "xlsm" | "xls" | "ods" => ler_excel(bytes)?,
        _ => return Err("use uma planilha .xlsx, .xls, .ods ou .csv".into()),
    };
    Ok(tabela)
}

fn montar_linhas(tabela: Vec<Vec<String>>) -> Result<Vec<LinhaPlanilha>, String> {
    let mut linhas = tabela.into_iter().enumerate();
    let (_, cabecalho) = linhas.next().ok_or("a planilha está vazia")?;
    let colunas: Vec<Option<Coluna>> = cabecalho.iter().map(|c| Coluna::do_cabecalho(c)).collect();
    for obrigatoria in [Coluna::Tomador, Coluna::Valor] {
        if !colunas.contains(&Some(obrigatoria)) {
            return Err(format!(
                "a primeira linha precisa ter a coluna \"{}\" (baixe o modelo para ver os nomes aceitos)",
                obrigatoria.titulo()
            ));
        }
    }
    Ok(linhas
        .filter_map(|(i, celulas)| {
            let campos: BTreeMap<Coluna, String> = colunas
                .iter()
                .zip(celulas)
                .filter_map(|(coluna, valor)| Some(((*coluna)?, valor.trim().to_owned())))
                .filter(|(_, v)| !v.is_empty())
                .collect();
            (!campos.is_empty()).then_some(LinhaPlanilha { numero: i + 1, campos })
        })
        .collect())
}

fn ler_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let texto = match std::str::from_utf8(bytes) {
        Ok(t) => t.trim_start_matches('\u{feff}').to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    };
    let primeira = texto.lines().next().unwrap_or_default();
    let separador = if primeira.matches(';').count() >= primeira.matches(',').count() {
        b';'
    } else {
        b','
    };
    let mut leitor = csv::ReaderBuilder::new()
        .delimiter(separador)
        .has_headers(false)
        .flexible(true)
        .from_reader(texto.as_bytes());
    leitor
        .records()
        .map(|r| {
            r.map(|registro| registro.iter().map(str::to_owned).collect())
                .map_err(|e| format!("CSV inválido: {e}"))
        })
        .collect()
}

fn ler_excel(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut pasta = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("não foi possível ler a planilha: {e}"))?;
    let aba = pasta
        .worksheet_range_at(0)
        .ok_or("a planilha não tem abas")?
        .map_err(|e| format!("não foi possível ler a primeira aba: {e}"))?;
    Ok(aba.rows().map(|linha| linha.iter().map(celula).collect()).collect())
}

fn celula(dado: &Data) -> String {
    match dado {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        // Números inteiros (CNPJ digitado como número) sem casas. Os demais com as casas que têm
        // (sem o ruído de ponto flutuante de fórmulas): quem lê recusa mais de 2, como no CSV, em vez
        // de arredondar em silêncio. Célula formatada como % chega como fração (2% = 0.02).
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => {
            let texto = format!("{f:.6}");
            texto.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => d
            .as_datetime()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
    }
}

/// Modelo de planilha em CSV (separado por `;`, como o Excel brasileiro abre direto).
pub fn modelo_csv() -> String {
    let cabecalho: Vec<&str> = Coluna::TODAS.iter().map(|c| c.titulo()).collect();
    format!(
        "{}\r\n{}\r\n",
        cabecalho.join(";"),
        "11222333000181;12ABC34501DE35;1.250,00;170101;SERVIÇOS PRESTADOS REFERENTE A {mes_ref};2026-09;Pedido 123"
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn csv_com_ponto_e_virgula_acentos_e_sinonimos() {
        let csv = "CNPJ do Tomador;Valor do Serviço;Descrição;Coluna extra\r\n\
                   12.ABC.345/01DE-35;1.250,00;CONSULTAS {mes_ref};x\r\n\
                   ;;;\r\n\
                   12345678909;80,5;;\r\n";
        let linhas = ler("convenio.csv", csv.as_bytes()).unwrap();
        assert_eq!(linhas.len(), 2, "linha vazia é ignorada");
        assert_eq!(linhas[0].numero, 2);
        assert_eq!(linhas[0].campo(Coluna::Tomador), Some("12.ABC.345/01DE-35"));
        assert_eq!(linhas[0].campo(Coluna::Valor), Some("1.250,00"));
        assert_eq!(linhas[0].campo(Coluna::Descricao), Some("CONSULTAS {mes_ref}"));
        assert_eq!(linhas[1].numero, 4);
        assert_eq!(linhas[1].campo(Coluna::Descricao), None);
    }

    #[test]
    fn csv_windows_1252_e_virgula() {
        // "Descrição" e "CONSULTÓRIO" em Windows-1252, separado por vírgula.
        let mut bytes = b"Tomador,Valor,Descri\xe7\xe3o\r\n".to_vec();
        bytes.extend_from_slice(b"12345678909,\"80,50\",CONSULT\xd3RIO\r\n");
        let linhas = ler("x.CSV", &bytes).unwrap();
        assert_eq!(linhas[0].campo(Coluna::Valor), Some("80,50"));
        assert_eq!(linhas[0].campo(Coluna::Descricao), Some("CONSULTÓRIO"));
    }

    #[test]
    fn exige_colunas_e_formato() {
        assert!(ler("x.csv", b"Nome;Valor\r\nA;1\r\n").unwrap_err().contains("tomador"));
        assert!(ler("x.pdf", b"").is_err());
        assert!(ler("x.csv", b"").is_err());
    }

    #[test]
    fn modelo_e_lido_de_volta() {
        let linhas = ler("modelo.csv", modelo_csv().as_bytes()).unwrap();
        assert_eq!(linhas.len(), 1);
        assert_eq!(linhas[0].campos.len(), Coluna::TODAS.len());
    }

    #[test]
    fn celulas_do_excel() {
        assert_eq!(celula(&Data::Float(191.0)), "191");
        assert_eq!(celula(&Data::Float(1250.5)), "1250.5");
        assert_eq!(celula(&Data::Float(0.0275)), "0.0275");
        assert_eq!(celula(&Data::Float(1250.4999999999998)), "1250.5");
        assert_eq!(celula(&Data::Int(11222333000181)), "11222333000181");
        assert_eq!(celula(&Data::Empty), "");
    }
}

#[cfg(test)]
mod testes_xlsx {
    use std::io::Write;

    use super::*;

    /// XLSX mínimo (uma aba, textos em linha e números), como o Excel grava.
    fn xlsx(linhas: &[&[&str]]) -> Vec<u8> {
        let celula = |ref_: String, v: &str| match v.parse::<f64>() {
            Ok(_) => format!(r#"<c r="{ref_}"><v>{v}</v></c>"#),
            Err(_) => format!(r#"<c r="{ref_}" t="inlineStr"><is><t>{v}</t></is></c>"#),
        };
        let dados: String = linhas
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let cels: String = l
                    .iter()
                    .enumerate()
                    .map(|(j, v)| celula(format!("{}{}", (b'A' + j as u8) as char, i + 1), v))
                    .collect();
                format!(r#"<row r="{}">{cels}</row>"#, i + 1)
            })
            .collect();
        let arquivos = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_owned(),
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_owned(),
            ),
            (
                "xl/workbook.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Notas" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_owned(),
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_owned(),
            ),
            (
                "xl/worksheets/sheet1.xml",
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{dados}</sheetData></worksheet>"#
                ),
            ),
        ];
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opcoes = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (nome, conteudo) in arquivos {
            zip.start_file(nome, opcoes).unwrap();
            zip.write_all(conteudo.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn le_xlsx_com_numeros() {
        let bytes = xlsx(&[
            &["CNPJ/CPF tomador", "Valor", "Mês referência"],
            // CNPJ digitado como número (perde os zeros) e valor numérico.
            &["191", "1250.5", "09/2026"],
        ]);
        let linhas = ler("convenio.xlsx", &bytes).unwrap();
        assert_eq!(linhas.len(), 1);
        assert_eq!(linhas[0].numero, 2);
        assert_eq!(linhas[0].campo(Coluna::Tomador), Some("191"));
        assert_eq!(linhas[0].campo(Coluna::Valor), Some("1250.5"));
        assert_eq!(
            crate::importacao::valor_da_planilha("1250.5"),
            Some(nfse_core::Valor::parse("1250.50").unwrap())
        );
        assert_eq!(linhas[0].campo(Coluna::Mes), Some("09/2026"));
    }
}
