//! Diálogos nativos do sistema.

use std::path::PathBuf;

use super::comum::{R, texto};

/// Abre o seletor nativo de pastas, começando em `inicial` quando ela existe. `None` se o operador
/// cancelar. O caminho escolhido só preenche o campo: quem grava (e valida) é `definir_pasta`.
#[tauri::command]
pub async fn escolher_pasta(app: tauri::AppHandle, inicial: String) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialogo = app.dialog().file().set_title("Escolher pasta");
    let inicial = PathBuf::from(inicial.trim());
    if !inicial.as_os_str().is_empty() && inicial.is_dir() {
        dialogo = dialogo.set_directory(inicial);
    }
    let (envio, recebimento) = tokio::sync::oneshot::channel();
    dialogo.pick_folder(move |pasta| {
        let _ = envio.send(pasta);
    });
    let pasta = recebimento.await.map_err(texto)?;
    pasta
        .map(|p| p.into_path().map(|c| c.display().to_string()).map_err(texto))
        .transpose()
}
