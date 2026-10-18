//! Gera `dados/lista-servicos-nacional.tsv` (aba `LISTA.SERV.NAC.`) e `dados/nbs.tsv` (aba
//! `LISTA.NBS_v2.0`) a partir do Anexo B da documentação técnica da NFS-e Nacional. Uso:
//!
//! ```text
//! cargo run -p nfse-core --example gerar_lista_servicos -- caminho/anexo_b-...xlsx
//! ```
//!
//! O nome do arquivo de origem vai para a primeira linha de cada TSV (`# fonte: ...`) e aparece na tela
//! como versão da lista. Ver docs/desenvolvimento.md.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use calamine::{Data, Reader};

const ABA: &str = "LISTA.SERV.NAC.";
const ABA_NBS: &str = "LISTA.NBS_v2.0";

fn main() -> Result<(), String> {
    let origem = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("informe o caminho da planilha do Anexo B (.xlsx)")?;
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/lista-servicos-nacional.tsv");

    let mut pasta = calamine::open_workbook_auto(&origem).map_err(|e| format!("{}: {e}", origem.display()))?;
    let aba = pasta.worksheet_range(ABA).map_err(|e| format!("aba {ABA}: {e}"))?;

    let nome = origem
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut tsv = format!("# fonte: {nome}\n");
    let mut grupo = String::new();
    let mut quantos = 0;
    // cabeçalho: CÓDIGO | ITEM | SUBITEM | DESDOBRO | DESCRIÇÃO
    for (i, linha) in aba.rows().enumerate().skip(1) {
        let campo = |c: usize| linha.get(c).map(celula).unwrap_or_default();
        let (item, subitem, desdobro) = (numero(&campo(1)), numero(&campo(2)), numero(&campo(3)));
        let descricao = campo(4).split_whitespace().collect::<Vec<_>>().join(" ");
        let (Some(item), Some(subitem), Some(desdobro)) = (item, subitem, desdobro) else {
            if linha.iter().all(|d| matches!(d, Data::Empty)) {
                continue;
            }
            return Err(format!("linha {}: item/subitem/desdobro não numéricos", i + 1));
        };
        if subitem == 0 {
            grupo = descricao;
            continue;
        }
        if desdobro == 0 {
            continue; // o subitem da LC 116; a nota usa o desdobro
        }
        let codigo = format!("{item:02}{subitem:02}{desdobro:02}");
        let informado = campo(0);
        if !informado.is_empty() && format!("{informado:0>6}") != codigo {
            return Err(format!("linha {}: código {informado} não bate com {codigo}", i + 1));
        }
        writeln!(tsv, "{codigo}\t{descricao}\t{grupo}").expect("escrever em String");
        quantos += 1;
    }
    std::fs::write(&destino, tsv).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("{quantos} códigos gravados em {}", destino.display());

    let aba = pasta
        .worksheet_range(ABA_NBS)
        .map_err(|e| format!("aba {ABA_NBS}: {e}"))?;
    gerar_nbs(&aba, &nome)
}

/// NBS: só os códigos de 9 dígitos (os do `cNBS`); o grupo é a descrição do nível mais próximo
/// acima (posição, subposição, item ou subitem), achado pelo prefixo dos dígitos, ou vazio.
fn gerar_nbs(aba: &calamine::Range<Data>, nome: &str) -> Result<(), String> {
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/nbs.tsv");
    let mut tsv = format!("# fonte: {nome}\n");
    // níveis acima do código atual: (dígitos, descrição), do mais largo ao mais estreito
    let mut niveis: Vec<(String, String)> = Vec::new();
    let mut quantos = 0;
    // cabeçalho: CÓDIGO NBS | DESCRIÇÃO
    for (i, linha) in aba.rows().enumerate().skip(1) {
        let campo = |c: usize| linha.get(c).map(celula).unwrap_or_default();
        let codigo: String = campo(0).chars().filter(char::is_ascii_digit).collect();
        let descricao = campo(1).split_whitespace().collect::<Vec<_>>().join(" ");
        if codigo.is_empty() {
            if linha.iter().all(|d| matches!(d, Data::Empty)) {
                continue;
            }
            return Err(format!("aba {ABA_NBS}, linha {}: sem código", i + 1));
        }
        if descricao.is_empty() {
            continue; // o fim da aba traz linhas de modelo (`9.9999.99.99`) sem descrição
        }
        niveis.retain(|(d, _)| codigo.starts_with(d.as_str()) && d.len() < codigo.len());
        if codigo.len() < 9 {
            niveis.push((codigo, descricao));
            continue;
        }
        if codigo.len() > 9 {
            return Err(format!(
                "aba {ABA_NBS}, linha {}: código {codigo} com mais de 9 dígitos",
                i + 1
            ));
        }
        // posição sem subdivisões (X.XXXX.00.00) já é o código completo: não tem nível acima na aba
        let grupo = niveis.last().map_or("", |(_, d)| d.as_str());
        writeln!(tsv, "{codigo}\t{descricao}\t{grupo}").expect("escrever em String");
        quantos += 1;
    }
    std::fs::write(&destino, tsv).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("{quantos} códigos NBS gravados em {}", destino.display());
    Ok(())
}

fn numero(t: &str) -> Option<u32> {
    t.trim().parse().ok()
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
