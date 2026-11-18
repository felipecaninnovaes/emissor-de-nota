//! Municípios do IBGE (código de 7 dígitos, nome e UF), embutidos a partir da DTB (Divisão
//! Territorial Brasileira). O arquivo `dados/municipios.tsv` é gerado pelo exemplo
//! `gerar_municipios`; a nota continua recebendo só o código.

use std::sync::OnceLock;

use serde::Serialize;

use crate::texto::sem_acento_minusculo;

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
    /// Nome normalizado de cada município para a busca (mesma ordem de `itens`).
    nomes: Vec<String>,
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
        let nomes = itens.iter().map(|m| normalizar(m.nome)).collect();
        Tabela { fonte, itens, nomes }
    })
}

/// Sem acento, minúsculo e com hífen, apóstrofo e outra pontuação como espaço:
/// "Embu-Guaçu" → "embu guacu", "Alta Floresta D'Oeste" → "alta floresta d oeste".
fn normalizar(texto: &str) -> String {
    let texto: String = sem_acento_minusculo(texto)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// Busca por nome ou código, para responder enquanto o operador digita. Só dígitos: códigos que
/// começam com eles. Texto: municípios cujo nome tem todas as palavras, sem diferenciar acento,
/// maiúscula nem pontuação; se a última palavra é uma UF ("ribeirao preto sp"), vale também o
/// município dessa UF com as demais palavras no nome, e ele vem primeiro. Depois, nome igual ao
/// termo, nome que começa com ele, palavra que começa com ele e o resto, em ordem de nome.
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static Municipio> {
    let tabela = tabela();
    let termo = termo.trim();
    // só pontuação ("-", ".") não é código: cai no texto, que fica vazio
    if termo.chars().any(|c| c.is_ascii_digit())
        && termo
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == ' ')
    {
        let digitos: String = termo.chars().filter(char::is_ascii_digit).collect();
        return tabela
            .itens
            .iter()
            .filter(|m| m.codigo.starts_with(&digitos))
            .take(limite)
            .collect();
    }
    let termo = normalizar(termo);
    let palavras: Vec<&str> = termo.split_whitespace().collect();
    if palavras.is_empty() {
        return Vec::new();
    }
    // "bom jesus rs": a última palavra pode ser a UF
    let uf_pedida = match palavras.as_slice() {
        [resto @ .., uf] if !resto.is_empty() && uf.len() == 2 => Some((uf.to_ascii_uppercase(), resto.join(" "))),
        _ => None,
    };
    let mut achados: Vec<(u8, u8, &str, usize)> = Vec::new();
    for (p, (m, nome)) in tabela.itens.iter().zip(&tabela.nomes).enumerate() {
        let pela_uf = uf_pedida
            .as_ref()
            .filter(|(uf, resto)| m.uf == uf && resto.split(' ').all(|w| nome.contains(w)));
        let alvo = match pela_uf {
            Some((_, resto)) => resto.as_str(),
            None if palavras.iter().all(|w| nome.contains(w)) => termo.as_str(),
            None => continue,
        };
        let nivel = if nome == alvo {
            0
        } else if nome.starts_with(alvo) {
            1
        } else if nome
            // aqui o nome não começa com o alvo: toda ocorrência tem um caractere antes
            .match_indices(alvo)
            .any(|(i, _)| nome.as_bytes()[i - 1] == b' ')
        {
            2
        } else {
            3
        };
        achados.push((u8::from(pela_uf.is_none()), nivel, nome, p));
    }
    achados.sort_unstable();
    achados
        .into_iter()
        .take(limite)
        .map(|(_, _, _, p)| &tabela.itens[p])
        .collect()
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

    fn primeiro(termo: &str) -> Option<(&'static str, &'static str)> {
        buscar(termo, 30).first().map(|m| (m.codigo, m.uf))
    }

    #[test]
    fn nome_sem_acento_traz_o_municipio_primeiro() {
        assert_eq!(primeiro("ribeirao preto"), Some(("3543402", "SP")));
        // São Paulo das Missões (RS) tem código menor, mas o nome igual vem antes
        assert_eq!(primeiro("sao paulo"), Some(("3550308", "SP")));
    }

    #[test]
    fn nome_com_acento_e_maiuscula() {
        for termo in ["SÃO JOSÉ DOS CAMPOS", "sao jose dos campos", "  São José   dos Campos "] {
            assert_eq!(primeiro(termo), Some(("3549904", "SP")), "{termo}");
        }
        // hífen e apóstrofo valem como espaço
        assert!(buscar("embu guacu", 30).iter().any(|m| m.nome == "Embu-Guaçu"));
        assert!(
            buscar("alta floresta d oeste", 30)
                .iter()
                .any(|m| m.codigo == "1100015")
        );
    }

    #[test]
    fn busca_pelo_codigo() {
        let achados = buscar("3543402", 30);
        assert_eq!(achados.len(), 1);
        assert_eq!((achados[0].nome, achados[0].uf), ("Ribeirão Preto", "SP"));
        assert!(buscar("35", 1000).iter().all(|m| m.codigo.starts_with("35")));
    }

    #[test]
    fn homonimos_com_a_uf_e_uf_no_fim_do_termo() {
        let achados = buscar("bom jesus", 30);
        let ufs: Vec<_> = achados.iter().filter(|m| m.nome == "Bom Jesus").map(|m| m.uf).collect();
        assert!(ufs.len() > 1 && ufs.contains(&"PI") && ufs.contains(&"RS"), "{ufs:?}");
        assert_eq!(primeiro("bom jesus rs"), Some(("4302303", "RS")));
        assert_eq!(primeiro("Ribeirão Preto SP"), Some(("3543402", "SP")));
    }

    #[test]
    fn termo_vazio_sem_correspondencia_e_curto() {
        assert!(buscar("", 30).is_empty());
        assert!(buscar("   ", 30).is_empty());
        assert!(buscar("xyzwq", 30).is_empty());
        assert!(buscar("-'", 30).is_empty());
        // pontuação de código sem nenhum dígito não lista a tabela inteira
        for termo in ["-", ".", " - ", "..-"] {
            assert!(buscar(termo, 30).is_empty(), "{termo:?}");
        }
        assert_eq!(buscar("a", 30).len(), 30);
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
