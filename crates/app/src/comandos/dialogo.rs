//! Diálogos nativos do sistema (no Linux, pelo xdg-desktop-portal).

use std::path::PathBuf;

use serde::Serialize;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use super::comum::{R, texto};

/// Diálogo com título e, se houver, filtro de extensões (sem o ponto) e pasta inicial existente.
fn dialogo(
    app: &tauri::AppHandle,
    titulo: &str,
    filtro: &str,
    extensoes: &[String],
    inicial: &str,
) -> FileDialogBuilder<tauri::Wry> {
    let mut dialogo = app.dialog().file().set_title(titulo);
    if !extensoes.is_empty() {
        let extensoes: Vec<&str> = extensoes.iter().map(String::as_str).collect();
        dialogo = dialogo.add_filter(filtro, &extensoes);
    }
    let inicial = PathBuf::from(inicial.trim());
    let pasta = if inicial.is_dir() {
        Some(inicial.as_path())
    } else {
        inicial.parent()
    };
    if let Some(pasta) = pasta.filter(|p| !p.as_os_str().is_empty() && p.is_dir()) {
        dialogo = dialogo.set_directory(pasta);
    }
    dialogo
}

/// Espera a resposta do diálogo (que chega por callback); `None` se o operador cancelar.
async fn resposta(abrir: impl FnOnce(tokio::sync::oneshot::Sender<Option<FilePath>>)) -> R<Option<PathBuf>> {
    let (envio, recebimento) = tokio::sync::oneshot::channel();
    abrir(envio);
    let caminho = recebimento.await.map_err(texto)?;
    caminho.map(|c| c.into_path().map_err(texto)).transpose()
}

/// Abre o seletor nativo de pastas, começando em `inicial` quando ela existe. `None` se o operador
/// cancelar. O caminho escolhido só preenche o campo: quem grava (e valida) é `definir_pasta`.
#[tauri::command]
pub async fn escolher_pasta(app: tauri::AppHandle, inicial: String) -> R<Option<String>> {
    let dialogo = dialogo(&app, "Escolher pasta", "", &[], &inicial);
    let pasta = resposta(|envio| {
        dialogo.pick_folder(move |p| {
            let _ = envio.send(p);
        })
    })
    .await?;
    Ok(pasta.map(|p| p.display().to_string()))
}

/// Seletor de um arquivo existente; devolve só o caminho (quem usa o arquivo é que o abre e valida).
#[tauri::command]
pub async fn escolher_arquivo(
    app: tauri::AppHandle,
    titulo: String,
    filtro: String,
    extensoes: Vec<String>,
    inicial: String,
) -> R<Option<String>> {
    let dialogo = dialogo(&app, &titulo, &filtro, &extensoes, &inicial);
    let arquivo = resposta(|envio| {
        dialogo.pick_file(move |a| {
            let _ = envio.send(a);
        })
    })
    .await?;
    Ok(arquivo.map(|a| a.display().to_string()))
}

#[derive(Serialize)]
pub struct ArquivoAberto {
    nome: String,
    bytes: Vec<u8>,
}

/// Seletor de um arquivo existente, devolvendo nome e conteúdo (certificado e planilhas, que os
/// comandos recebem em bytes). Só lê o arquivo que o operador escolheu no diálogo.
#[tauri::command]
pub async fn ler_arquivo_escolhido(
    app: tauri::AppHandle,
    titulo: String,
    filtro: String,
    extensoes: Vec<String>,
) -> R<Option<ArquivoAberto>> {
    let dialogo = dialogo(&app, &titulo, &filtro, &extensoes, "");
    let Some(caminho) = resposta(|envio| {
        dialogo.pick_file(move |a| {
            let _ = envio.send(a);
        })
    })
    .await?
    else {
        return Ok(None);
    };
    let bytes = tokio::fs::read(&caminho)
        .await
        .map_err(|e| format!("não foi possível ler {}: {e}", caminho.display()))?;
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(ArquivoAberto { nome, bytes }))
}

/// Seletor de "salvar como": devolve o caminho escolhido (o diálogo pergunta antes de sobrescrever;
/// quem grava confere de novo). Começa na pasta de `inicial` (pasta ou arquivo), com o `nome` sugerido.
#[tauri::command]
pub async fn salvar_arquivo(
    app: tauri::AppHandle,
    titulo: String,
    filtro: String,
    extensoes: Vec<String>,
    inicial: String,
    nome: String,
) -> R<Option<String>> {
    let dialogo = dialogo(&app, &titulo, &filtro, &extensoes, &inicial).set_file_name(nome);
    let arquivo = resposta(|envio| {
        dialogo.save_file(move |a| {
            let _ = envio.send(a);
        })
    })
    .await?;
    Ok(arquivo.map(|a| a.display().to_string()))
}
