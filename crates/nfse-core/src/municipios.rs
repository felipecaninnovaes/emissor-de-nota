//! Municípios do IBGE (código de 7 dígitos, nome e UF), embutidos a partir da DTB (Divisão
//! Territorial Brasileira). O arquivo `dados/municipios.tsv` é gerado pelo exemplo
//! `gerar_municipios`; a nota continua recebendo só o código.

use std::sync::OnceLock;

use serde::Serialize;

const TSV: &str = include_str!("../dados/municipios.tsv");

/// Um município da tabela embutida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Municipio {
    /// Código IBGE com 7 dígitos (ex.: `3550308`).
    pub codigo: &'static str,
    pub nome: &'static str,
    /// Sigla da UF (ex.: `SP`).
    pub uf: &'static str,
}

struct Tabela {
    fonte: &'static str,
    itens: Vec<Municipio>,
}

fn tabela() -> &'static Tabela {
    static TABELA: OnceLock<Tabela> = OnceLock::new();
    TABELA.get_or_init(|| {
        let mut fonte = "";
        let mut itens = Vec::new();
        for linha in TSV.lines().map(|l| l.trim_end_matches('\r')) {
            if let Some(f) = linha.strip_prefix("# fonte:") {
                fonte = f.trim();
                continue;
            }
            let mut campos = linha.split('\t');
            if let (Some(codigo), Some(nome), Some(uf)) = (campos.next(), campos.next(), campos.next()) {
                itens.push(Municipio { codigo, nome, uf });
            }
        }
        Tabela { fonte, itens }
    })
}

/// Nome do arquivo da DTB de onde a tabela embutida foi gerada (indica a versão).
pub fn fonte() -> &'static str {
    tabela().fonte
}

/// Todos os municípios, em ordem de código.
pub fn itens() -> &'static [Municipio] {
    &tabela().itens
}

pub fn por_codigo(codigo: &str) -> Option<&'static Municipio> {
    let itens = itens();
    itens.binary_search_by(|m| m.codigo.cmp(codigo)).ok().map(|p| &itens[p])
}

/// UF a partir dos dois primeiros dígitos do código IBGE. O `nfse-pdf` tem uma cópia: ele não
/// depende do `nfse-core` (ver docs/arquitetura.md).
pub fn uf_do_ibge(codigo: &str) -> Option<&'static str> {
    Some(match codigo.get(0..2)? {
        "11" => "RO",
        "12" => "AC",
        "13" => "AM",
        "14" => "RR",
        "15" => "PA",
        "16" => "AP",
        "17" => "TO",
        "21" => "MA",
        "22" => "PI",
        "23" => "CE",
        "24" => "RN",
        "25" => "PB",
        "26" => "PE",
        "27" => "AL",
        "28" => "SE",
        "29" => "BA",
        "31" => "MG",
        "32" => "ES",
        "33" => "RJ",
        "35" => "SP",
        "41" => "PR",
        "42" => "SC",
        "43" => "RS",
        "50" => "MS",
        "51" => "MT",
        "52" => "GO",
        "53" => "DF",
        _ => return None,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Total de municípios na DTB de origem (o gerador imprime este número).
    const TOTAL_DA_DTB: usize = 5571;

    #[test]
    fn sao_paulo_pelo_codigo_e_fonte() {
        assert_eq!(fonte(), "RELATORIO_DTB_BRASIL_2025_MUNICIPIOS.xls");
        let sp = por_codigo("3550308").unwrap();
        assert_eq!((sp.nome, sp.uf), ("São Paulo", "SP"));
        assert!(por_codigo("9999999").is_none());
    }

    #[test]
    fn tabela_embutida_e_valida() {
        let itens = itens();
        assert_eq!(itens.len(), TOTAL_DA_DTB);
        for janela in itens.windows(2) {
            assert!(
                janela[0].codigo < janela[1].codigo,
                "códigos fora de ordem ou repetidos: {}",
                janela[1].codigo
            );
        }
        for m in itens {
            assert!(
                m.codigo.len() == 7 && m.codigo.bytes().all(|b| b.is_ascii_digit()),
                "{}",
                m.codigo
            );
            assert!(!m.nome.is_empty(), "{}", m.codigo);
            assert_eq!(Some(m.uf), uf_do_ibge(m.codigo), "{}", m.codigo);
        }
    }
}
