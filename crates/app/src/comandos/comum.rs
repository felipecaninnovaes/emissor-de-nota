//! Tipos e funções usados por mais de um grupo de comandos.

use std::path::PathBuf;

use chrono::NaiveDate;

use crate::Estado;

pub(crate) type R<T> = Result<T, String>;

pub(crate) fn texto<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub(crate) const CHAVE_PASTA: &str = "pasta_saida";

/// Pasta padrão: Documentos\NFS-e do usuário.
fn pasta_padrao() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Documents").join("NFS-e")
}

pub(crate) async fn pasta_configurada(estado: &Estado) -> R<PathBuf> {
    Ok(estado
        .banco()?
        .configuracao(CHAVE_PASTA)
        .await
        .map_err(texto)?
        .map(PathBuf::from)
        .unwrap_or_else(pasta_padrao))
}

/// Abre o arquivo ou pasta com o programa padrão do sistema.
///
/// No Linux, nem toda instalação traz o `xdg-open` (pacote xdg-utils); sem ele,
/// tenta o `gio open`, que vem com o GLib.
pub(crate) fn abrir_no_sistema(caminho: &std::path::Path) -> R<()> {
    let candidatos: &[(&str, &[&str])] = if cfg!(windows) {
        &[("explorer", &[])]
    } else if cfg!(target_os = "macos") {
        &[("open", &[])]
    } else {
        &[("xdg-open", &[]), ("gio", &["open"])]
    };
    let mut ultimo_erro = None;
    for (programa, argumentos) in candidatos {
        match std::process::Command::new(programa)
            .args(*argumentos)
            .arg(caminho)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ultimo_erro = Some(format!("programa {programa} não encontrado"));
            }
            Err(e) => ultimo_erro = Some(format!("{programa}: {e}")),
        }
    }
    Err(format!(
        "não foi possível abrir {}: {}",
        caminho.display(),
        ultimo_erro.unwrap_or_default()
    ))
}

/// Os dados fiscais só são acessíveis com o cofre aberto pela senha mestra.
pub(crate) async fn exigir_cofre(estado: &Estado) -> R<()> {
    if estado.emissor.read().await.is_none() {
        return Err("abra o sistema com a senha mestra".into());
    }
    Ok(())
}

macro_rules! emissor {
    ($estado:expr) => {{
        let guarda = $estado.emissor.read().await;
        if guarda.is_none() {
            return Err("abra o sistema com a senha mestra".into());
        }
        guarda
    }};
}
pub(crate) use emissor;

pub(crate) fn vazio_para_none(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub(crate) fn mes_de(mes: &str) -> R<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{}-01", mes.trim()), "%Y-%m-%d")
        .map_err(|_| "mês de referência inválido".into())
}

/// Grava o CSV na pasta das notas e abre no programa padrão (Excel). Devolve o caminho.
pub(crate) async fn gravar_e_abrir_csv(estado: &Estado, nome: &str, conteudo: &str) -> R<String> {
    let pasta = pasta_configurada(estado).await?;
    std::fs::create_dir_all(&pasta).map_err(texto)?;
    let caminho = pasta.join(nome);
    // BOM: o Excel abre o CSV em UTF-8 com os acentos certos.
    std::fs::write(&caminho, format!("\u{feff}{conteudo}"))
        .map_err(|e| format!("não foi possível gravar {nome}: {e}"))?;
    abrir_no_sistema(&caminho)?;
    Ok(caminho.display().to_string())
}
