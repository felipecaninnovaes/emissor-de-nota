//! Gera `dados/municipios.tsv` a partir da planilha de municípios da DTB (Divisão Territorial
//! Brasileira) do IBGE. Uso:
//!
//! ```text
//! cargo run -p nfse-core --example gerar_municipios -- caminho/RELATORIO_DTB_BRASIL_AAAA_MUNICIPIOS.xls
//! ```
//!
//! O nome do arquivo de origem vai para a primeira linha do TSV (`# fonte: ...`) e aparece na tela
//! como versão da tabela. Ver docs/desenvolvimento.md.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use calamine::{Data, Reader};
use nfse_core::municipios::uf_do_ibge;

const COLUNA_CODIGO: &str = "Código Município Completo";
const COLUNA_NOME: &str = "Nome_Município";

fn main() -> Result<(), String> {
    let origem = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("informe o caminho da planilha de municípios da DTB (.xls)")?;
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/municipios.tsv");

    let mut pasta = calamine::open_workbook_auto(&origem).map_err(|e| format!("{}: {e}", origem.display()))?;
    let aba = pasta
        .worksheet_range_at(0)
        .ok_or("a planilha não tem abas")?
        .map_err(|e| format!("primeira aba: {e}"))?;

    // o cabeçalho não fica na primeira linha: procura pelas colunas pelo nome
    let linhas: Vec<&[Data]> = aba.rows().collect();
    let (inicio, codigo_col, nome_col) = linhas
        .iter()
        .enumerate()
        .find_map(|(i, linha)| {
            let coluna = |nome: &str| linha.iter().position(|d| celula(d) == nome);
            Some((i + 1, coluna(COLUNA_CODIGO)?, coluna(COLUNA_NOME)?))
        })
        .ok_or(format!(
            "cabeçalho com \"{COLUNA_CODIGO}\" e \"{COLUNA_NOME}\" não encontrado"
        ))?;

    let mut municipios = BTreeMap::new();
    for (i, linha) in linhas.iter().enumerate().skip(inicio) {
        let campo = |c: usize| linha.get(c).map(celula).unwrap_or_default();
        let (codigo, nome) = (campo(codigo_col), campo(nome_col));
        if codigo.is_empty() && nome.is_empty() {
            continue;
        }
        let nome = nome.split_whitespace().collect::<Vec<_>>().join(" ");
        if codigo.len() != 7 || !codigo.bytes().all(|b| b.is_ascii_digit()) || nome.is_empty() {
            return Err(format!(
                "linha {}: código \"{codigo}\" ou nome \"{nome}\" inválido",
                i + 1
            ));
        }
        let uf = uf_do_ibge(&codigo).ok_or(format!("linha {}: UF desconhecida no código {codigo}", i + 1))?;
        if municipios.insert(codigo.clone(), (nome, uf)).is_some() {
            return Err(format!("linha {}: código {codigo} repetido", i + 1));
        }
    }

    let nome = origem
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut tsv = format!("# fonte: {nome}\n");
    for (codigo, (nome, uf)) in &municipios {
        writeln!(tsv, "{codigo}\t{nome}\t{uf}").expect("escrever em String");
    }
    std::fs::write(&destino, tsv).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("{} municípios gravados em {}", municipios.len(), destino.display());
    Ok(())
}

fn celula(dado: &Data) -> String {
    match dado {
        Data::String(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        _ => String::new(),
    }
}
