//! Diálogos nativos do sistema (no Linux, pelo xdg-desktop-portal).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use super::comum::{R, texto};
use crate::Estado;

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
    /// Identifica o arquivo para `reler_arquivo_escolhido`.
    id: usize,
    nome: String,
    bytes: Vec<u8>,
}

/// Lê do disco o arquivo escolhido (o conteúdo atual, mesmo que tenha mudado desde a escolha).
async fn abrir(id: usize, caminho: &Path) -> R<ArquivoAberto> {
    let bytes = tokio::fs::read(caminho)
        .await
        .map_err(|e| format!("não foi possível ler {}: {e}", caminho.display()))?;
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(ArquivoAberto { id, nome, bytes })
}

/// Guarda o caminho escolhido no diálogo e devolve o número dele.
fn registrar(escolhidos: &Mutex<Vec<PathBuf>>, caminho: PathBuf) -> usize {
    let mut lista = escolhidos.lock().unwrap_or_else(|e| e.into_inner());
    lista.push(caminho);
    lista.len() - 1
}

/// Caminho de um arquivo já escolhido no diálogo; erro se o número não for de nenhum.
fn escolhido(escolhidos: &Mutex<Vec<PathBuf>>, id: usize) -> R<PathBuf> {
    let lista = escolhidos.lock().unwrap_or_else(|e| e.into_inner());
    lista
        .get(id)
        .cloned()
        .ok_or_else(|| "escolha o arquivo de novo".to_string())
}

/// Seletor de um arquivo existente, devolvendo nome e conteúdo (certificado e planilhas, que os
/// comandos recebem em bytes). Só lê o arquivo que o operador escolheu no diálogo.
#[tauri::command]
pub async fn ler_arquivo_escolhido(
    app: tauri::AppHandle,
    estado: tauri::State<'_, Estado>,
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
    let id = registrar(&estado.arquivos_escolhidos, caminho.clone());
    abrir(id, &caminho).await.map(Some)
}

/// Lê de novo, do disco, um arquivo já escolhido em `ler_arquivo_escolhido` (o "Ler de novo" da
/// planilha, depois de o operador corrigir o arquivo). Só aceita arquivos escolhidos no diálogo.
#[tauri::command]
pub async fn reler_arquivo_escolhido(estado: tauri::State<'_, Estado>, id: usize) -> R<ArquivoAberto> {
    let caminho = escolhido(&estado.arquivos_escolhidos, id)?;
    abrir(id, &caminho).await
}

#[cfg(test)]
mod testes {
    use super::*;

    #[tokio::test]
    async fn reler_le_o_conteudo_atual_do_arquivo() {
        let pasta = std::env::temp_dir().join(format!("emissor-reler-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("notas.csv");
        std::fs::write(&arquivo, "antes").unwrap();
        let escolhidos = Mutex::default();
        let id = registrar(&escolhidos, arquivo.clone());
        assert_eq!(
            abrir(id, &escolhido(&escolhidos, id).unwrap()).await.unwrap().bytes,
            b"antes"
        );

        std::fs::write(&arquivo, "depois").unwrap();
        let relido = abrir(id, &escolhido(&escolhidos, id).unwrap()).await.unwrap();
        assert_eq!(relido.bytes, b"depois");
        assert_eq!(relido.nome, "notas.csv");
        // Limpeza do temporário; se falhar, só sobra um arquivo na pasta temporária.
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn reler_recusa_arquivo_nao_escolhido() {
        let escolhidos = Mutex::default();
        assert!(escolhido(&escolhidos, 0).is_err());
    }
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
