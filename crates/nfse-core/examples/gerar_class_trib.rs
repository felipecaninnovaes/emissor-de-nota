//! Gera `dados/class-trib.tsv` a partir da página "Classificação Tributária" do Portal DFe (SVRS),
//! que traz a tabela embutida em JSON (`var dadosOriginais = [...]`). Uso:
//!
//! ```text
//! curl -sL https://dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria -o class-trib.html
//! cargo run -p nfse-core --example gerar_class_trib -- class-trib.html
//! ```
//!
//! A primeira linha do TSV guarda a origem e a data da geração, que a tela mostra como versão.
//! Ver docs/desenvolvimento.md.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

const MARCA: &str = "var dadosOriginais = ";
const ORIGEM: &str = "dfe-portal.svrs.rs.gov.br/Cff/ClassificacaoTributaria";

fn main() -> Result<(), String> {
    let origem = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("informe o caminho da página HTML salva (class-trib.html)")?;
    let html = std::fs::read_to_string(&origem).map_err(|e| format!("{}: {e}", origem.display()))?;
    let inicio = html
        .find(MARCA)
        .ok_or("a página não tem `var dadosOriginais`: o portal mudou de formato")?
        + MARCA.len();
    let dados: Value = serde_json::Deserializer::from_str(&html[inicio..])
        .into_iter::<Value>()
        .next()
        .ok_or("JSON vazio")?
        .map_err(|e| format!("JSON da página: {e}"))?;

    let texto = |v: &Value, campo: &str| -> Result<String, String> {
        let t = v[campo]
            .as_str()
            .ok_or_else(|| format!("campo {campo} ausente em {v}"))?;
        Ok(t.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    let percentual = |v: &Value, campo: &str| -> Result<String, String> {
        let p = v[campo]
            .as_f64()
            .ok_or_else(|| format!("campo {campo} ausente em {v}"))?;
        Ok(p.to_string())
    };

    let mut linhas: Vec<(String, String)> = Vec::new();
    for cst in dados.as_array().ok_or("a tabela não é uma lista")? {
        let (codigo_cst, nome_cst) = (texto(cst, "Cst")?, texto(cst, "NomeCst")?);
        for c in cst["ClassificacoesTributarias"].as_array().into_iter().flatten() {
            let codigo = texto(c, "CodClassTrib")?;
            if codigo.len() != 6 || !codigo.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("cClassTrib inválido: {codigo}"));
            }
            if texto(c, "Cst")? != codigo_cst {
                return Err(format!("{codigo}: CST do código difere do grupo {codigo_cst}"));
            }
            let fim = c["DthFimVig"].as_str().map_or("", |d| d.get(..10).unwrap_or(d));
            let linha = format!(
                "{codigo}\t{}\t{nome_cst}\t{codigo_cst}\t{}\t{}\t{fim}",
                texto(c, "NomeClassTrib")?,
                percentual(c, "PercRedIbs")?,
                percentual(c, "PercRedCbs")?,
            );
            linhas.push((codigo, linha));
        }
    }
    linhas.sort();
    if linhas.windows(2).any(|j| j[0].0 == j[1].0) {
        return Err("cClassTrib repetido na tabela".into());
    }

    let hoje = chrono::Local::now().date_naive();
    let mut tsv = format!("# fonte: {ORIGEM} em {hoje}\n");
    for (_, linha) in &linhas {
        writeln!(tsv, "{linha}").expect("escrever em String");
    }
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/class-trib.tsv");
    std::fs::write(&destino, tsv).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("{} códigos gravados em {}", linhas.len(), destino.display());
    Ok(())
}
