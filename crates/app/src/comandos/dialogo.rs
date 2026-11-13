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

/// Lê de novo, do disco, um arquivo já escolhido em `ler_arquivo_escolhido` ou solto na janela
/// (`ao_soltar`): o "Ler de novo" da planilha, depois de o operador corrigir o arquivo. Só aceita
/// arquivos registrados pelo diálogo ou pelo evento de soltar do sistema.
#[tauri::command]
pub async fn reler_arquivo_escolhido(estado: tauri::State<'_, Estado>, id: usize) -> R<ArquivoAberto> {
    let caminho = escolhido(&estado.arquivos_escolhidos, id)?;
    abrir(id, &caminho).await
}

/// Extensões aceitas ao soltar um arquivo na tela Importar planilha (spec 0002). As mesmas do
/// `FILTRO_PLANILHA_EMAIL` de `ui/js/componentes.js`, que vale para o diálogo.
const EXTENSOES_SOLTAR: [&str; 7] = ["xlsx", "xlsm", "xls", "ods", "csv", "eml", "msg"];

/// O arquivo solto, se for um só, existir como arquivo e tiver extensão aceita (sem diferenciar
/// maiúsculas); senão, o aviso para o operador.
fn arquivo_solto(caminhos: &[PathBuf]) -> R<PathBuf> {
    let [caminho] = caminhos else {
        return Err("Solte um arquivo só.".into());
    };
    if !caminho.is_file() {
        return Err("Solte um arquivo só.".into());
    }
    let aceita = caminho
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSOES_SOLTAR.contains(&e.to_lowercase().as_str()));
    if !aceita {
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(format!("Tipo de arquivo não aceito: {nome}"));
    }
    Ok(caminho.clone())
}

/// O que a interface recebe no evento `arquivo-solto`: o número do arquivo registrado e o nome,
/// ou o aviso de recusa. Nunca o caminho nem o conteúdo (que ela lê por `reler_arquivo_escolhido`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
enum ArquivoSolto {
    Registrado { id: usize, nome: String },
    Recusado { erro: String },
}

/// Escolhe e registra o arquivo solto, na mesma lista dos escolhidos no diálogo.
fn registrar_solto(escolhidos: &Mutex<Vec<PathBuf>>, caminhos: &[PathBuf]) -> ArquivoSolto {
    match arquivo_solto(caminhos) {
        Ok(caminho) => {
            let nome = caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            ArquivoSolto::Registrado {
                id: registrar(escolhidos, caminho),
                nome,
            }
        }
        Err(erro) => ArquivoSolto::Recusado { erro },
    }
}

/// Arquivo solto na janela (evento do sistema, não da interface): registra e avisa a interface,
/// que decide se a tela aberta o usa (só a Importar planilha, fora da emissão).
pub fn ao_soltar<Rt: tauri::Runtime>(janela: &tauri::Window<Rt>, caminhos: &[PathBuf]) {
    use tauri::{Emitter, Manager};
    let Some(estado) = janela.try_state::<Estado>() else {
        return;
    };
    let solto = registrar_solto(&estado.arquivos_escolhidos, caminhos);
    if let Err(e) = janela.emit("arquivo-solto", solto) {
        tracing::warn!("não foi possível avisar a interface do arquivo solto: {e}");
    }
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

    /// Pasta temporária própria do teste, com os arquivos pedidos (conteúdo = nome).
    fn pasta_com(teste: &str, nomes: &[&str]) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!("emissor-solto-{teste}-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        for nome in nomes {
            std::fs::write(pasta.join(nome), nome).unwrap();
        }
        pasta
    }

    #[tokio::test]
    async fn solto_um_arquivo_e_registrado() {
        let pasta = pasta_com("um", &["pedidos.xlsx", "pedido.eml"]);
        let escolhidos = Mutex::default();
        for (esperado, nome) in ["pedidos.xlsx", "pedido.eml"].into_iter().enumerate() {
            let caminho = pasta.join(nome);
            let solto = registrar_solto(&escolhidos, std::slice::from_ref(&caminho));
            assert_eq!(
                solto,
                ArquivoSolto::Registrado {
                    id: esperado,
                    nome: nome.into()
                }
            );
            assert_eq!(escolhido(&escolhidos, esperado).unwrap(), caminho);
            let aberto = abrir(esperado, &caminho).await.unwrap();
            assert_eq!(aberto.bytes, nome.as_bytes());
        }
        assert_eq!(
            serde_json::to_value(registrar_solto(&escolhidos, &[pasta.join("pedidos.xlsx")])).unwrap(),
            serde_json::json!({ "id": 2, "nome": "pedidos.xlsx" })
        );
        // Limpeza do temporário; se falhar, só sobra um arquivo na pasta temporária.
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn solto_recusa_dois_pasta_e_pdf() {
        let pasta = pasta_com("recusa", &["a.csv", "b.csv", "nota.pdf"]);
        let escolhidos = Mutex::default();
        let casos = [
            (vec![pasta.join("a.csv"), pasta.join("b.csv")], "Solte um arquivo só."),
            (vec![pasta.clone()], "Solte um arquivo só."),
            (vec![], "Solte um arquivo só."),
            (vec![pasta.join("nota.pdf")], "Tipo de arquivo não aceito: nota.pdf"),
        ];
        for (caminhos, aviso) in casos {
            assert_eq!(
                registrar_solto(&escolhidos, &caminhos),
                ArquivoSolto::Recusado { erro: aviso.into() }
            );
        }
        assert!(escolhidos.lock().unwrap().is_empty());
        assert_eq!(
            serde_json::to_value(registrar_solto(&escolhidos, &[pasta.join("nota.pdf")])).unwrap(),
            serde_json::json!({ "erro": "Tipo de arquivo não aceito: nota.pdf" })
        );
        // Limpeza do temporário; se falhar, só sobra um arquivo na pasta temporária.
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn solto_aceita_extensao_maiuscula() {
        let pasta = pasta_com("maiuscula", &["PEDIDOS.CSV"]);
        let caminho = pasta.join("PEDIDOS.CSV");
        assert_eq!(arquivo_solto(std::slice::from_ref(&caminho)).unwrap(), caminho);
        // Limpeza do temporário; se falhar, só sobra um arquivo na pasta temporária.
        let _ = std::fs::remove_dir_all(&pasta);
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
