//! Pasta de saída por empresa e mês: `<raiz>/<NOME - CNPJ>/<AAAA-MM>/`, com o XML de cada nota
//! (e, depois, o PDF). É a pasta que o escritório usa para responder convênios e importar no Prosoft.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};

use nfse_db::empresas::Empresa;

/// Nome de pasta válido no Windows: sem `<>:"/\|?*` nem controle, sem ponto/espaço no fim, até 80 caracteres.
pub fn nome_seguro(texto: &str) -> String {
    let limpo: String = texto
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cortado: String = limpo.chars().take(80).collect();
    cortado.trim_end_matches(['.', ' ']).to_owned()
}

pub fn pasta_empresa(raiz: &Path, empresa: &Empresa) -> PathBuf {
    raiz.join(nome_seguro(&format!("{} - {}", empresa.nome, empresa.cnpj.as_str())))
}

pub fn pasta_mes(raiz: &Path, empresa: &Empresa, competencia: NaiveDate) -> PathBuf {
    pasta_empresa(raiz, empresa).join(format!("{:04}-{:02}", competencia.year(), competencia.month()))
}

pub fn nome_xml_nfse(numero: &str, chave: &str) -> String {
    format!("NFSe-{}-{}.xml", nome_seguro(numero), nome_seguro(chave))
}

pub fn nome_xml_evento(numero: &str, chave: &str, evento: &str) -> String {
    format!("NFSe-{}-{}-{evento}.xml", nome_seguro(numero), nome_seguro(chave))
}

/// Grava pelo caminho temporário + renomear, para nunca deixar um XML pela metade na pasta.
pub fn gravar(caminho: &Path, conteudo: impl AsRef<[u8]>) -> std::io::Result<()> {
    if let Some(pasta) = caminho.parent() {
        std::fs::create_dir_all(pasta)?;
    }
    let temporario = caminho.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&temporario)?;
        f.write_all(conteudo.as_ref())?;
        f.sync_all()?;
    }
    std::fs::rename(&temporario, caminho)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nomes_validos_no_windows() {
        assert_eq!(nome_seguro("A/B: C*D?  \"E\" <F>|G."), "A B C D E F G");
        assert_eq!(nome_seguro("CLÍNICA SÃO JOSÉ LTDA"), "CLÍNICA SÃO JOSÉ LTDA");
        assert_eq!(nome_seguro(&"X".repeat(100)).len(), 80);
    }

    #[test]
    fn grava_sem_deixar_temporario() {
        let dir = std::env::temp_dir().join(format!("nfse-arquivo-{}", std::process::id()));
        let caminho = dir.join("2026-10").join("NFSe-1-X.xml");
        gravar(&caminho, "<NFSe/>").unwrap();
        gravar(&caminho, "<NFSe>2</NFSe>").unwrap();
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), "<NFSe>2</NFSe>");
        assert_eq!(std::fs::read_dir(caminho.parent().unwrap()).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
