//! Gera `dados/lista-servicos-nacional.tsv` a partir do Anexo B da documentação técnica da NFS-e
//! Nacional (aba `LISTA.SERV.NAC.`). Uso:
//!
//! ```text
//! cargo run -p nfse-core --example gerar_lista_servicos -- caminho/anexo_b-...xlsx
//! ```
//!
//! O nome do arquivo de origem vai para a primeira linha do TSV (`# fonte: ...`) e aparece na tela
//! como versão da lista. Ver docs/desenvolvimento.md.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use calamine::{Data, Reader};

const ABA: &str = "LISTA.SERV.NAC.";

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
