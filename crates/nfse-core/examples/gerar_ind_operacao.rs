//! Gera `dados/ind-operacao.tsv` a partir da página "Tabela de Indicadores dos Locais de Operação"
//! do Portal DFe (SVRS), que traz a tabela em HTML. Uso:
//!
//! ```text
//! curl -sL https://dfe-portal.svrs.rs.gov.br/CFF/TabelaIndicadoresDosLocaisDeOperacao -o ind-operacao.html
//! cargo run -p nfse-core --example gerar_ind_operacao -- ind-operacao.html
//! ```
//!
//! A primeira linha do TSV guarda a origem e a data da geração, que a tela mostra como versão.
//! Ver docs/desenvolvimento.md.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const ORIGEM: &str = "dfe-portal.svrs.rs.gov.br/CFF/TabelaIndicadoresDosLocaisDeOperacao";
/// Colunas da tabela: código, nome, dispositivo legal, local, local do fornecedor, característica do
/// fornecedor, publicação, início e fim da vigência.
const COLUNAS: usize = 9;

fn main() -> Result<(), String> {
    let origem = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("informe o caminho da página HTML salva (ind-operacao.html)")?;
    let html = std::fs::read_to_string(&origem).map_err(|e| format!("{}: {e}", origem.display()))?;

    let mut linhas: Vec<(String, String)> = Vec::new();
    for tr in partes(&html, "<tr", "</tr>") {
        let celulas: Vec<String> = partes(tr, "<td", "</td>").map(texto).collect();
        let Some(codigo) = celulas.first() else { continue };
        if codigo.len() != 6 || !codigo.bytes().all(|b| b.is_ascii_digit()) {
            continue; // cabeçalho e linhas de título
        }
        if celulas.len() != COLUNAS {
            return Err(format!(
                "{codigo}: {} colunas em vez de {COLUNAS}: o portal mudou de formato",
                celulas.len()
            ));
        }
        let (nome, local_fornecedor) = (&celulas[1], &celulas[4]);
        if nome.is_empty() || local_fornecedor.is_empty() {
            return Err(format!("{codigo}: nome ou local vazio"));
        }
        let fim = celulas[8].trim_matches('-');
        if !fim.is_empty() {
            return Err(format!(
                "{codigo}: vigência encerrada em {fim}; trate no gerador antes de embutir"
            ));
        }
        linhas.push((codigo.clone(), format!("{codigo}\t{nome}\t{local_fornecedor}")));
    }
    if linhas.is_empty() {
        return Err("nenhum indicador na página: o portal mudou de formato".into());
    }
    linhas.sort();
    if linhas.windows(2).any(|j| j[0].0 == j[1].0) {
        return Err("cIndOp repetido na tabela".into());
    }

    let hoje = chrono::Local::now().date_naive();
    let mut tsv = format!("# fonte: {ORIGEM} em {hoje}\n");
    for (_, linha) in &linhas {
        writeln!(tsv, "{linha}").expect("escrever em String");
    }
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/ind-operacao.tsv");
    std::fs::write(&destino, tsv).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("{} indicadores gravados em {}", linhas.len(), destino.display());
    Ok(())
}

/// Trechos entre cada `abre` (início da tag, ex.: `<td`) e o próximo `fecha`.
fn partes<'a>(html: &'a str, abre: &'static str, fecha: &'static str) -> impl Iterator<Item = &'a str> {
    let mut resto = html;
    std::iter::from_fn(move || {
        let ini = resto.find(abre)?;
        let depois = &resto[ini..];
        let fim = depois.find(fecha)?;
        resto = &depois[fim + fecha.len()..];
        Some(&depois[..fim])
    })
}

/// Texto da célula: sem tags, com as entidades decodificadas e espaços normalizados.
fn texto(celula: &str) -> String {
    let mut limpo = String::new();
    let mut em_tag = false;
    for c in celula.chars() {
        match c {
            '<' => em_tag = true,
            '>' => em_tag = false,
            _ if !em_tag => limpo.push(c),
            _ => {}
        }
    }
    let mut saida = String::new();
    let mut resto = limpo.as_str();
    while let Some(i) = resto.find('&') {
        saida.push_str(&resto[..i]);
        let Some(fim) = resto[i..].find(';') else { break };
        let entidade = &resto[i + 1..i + fim];
        let decodificada = match entidade {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "nbsp" => Some(' '),
            _ => entidade
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entidade.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decodificada {
            Some(c) => saida.push(c),
            None => saida.push_str(&resto[i..=i + fim]),
        }
        resto = &resto[i + fim + 1..];
    }
    saida.push_str(resto);
    saida.split_whitespace().collect::<Vec<_>>().join(" ")
}
