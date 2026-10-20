//! Backup cifrado do banco: pasta de destino, backup manual e automático, restauração.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use nfse_db::backup;

use super::comum::{R, emissor, exigir_cofre, texto};
use crate::Estado;

const CHAVE_PASTA_BACKUP: &str = "backup_pasta";
const CHAVE_ULTIMO_BACKUP: &str = "backup_ultimo";
/// O backup automático roda ao abrir o sistema quando o último tem mais que isso.
const INTERVALO_AUTOMATICO: chrono::Duration = chrono::Duration::hours(20);
/// Banco restaurado que espera a próxima abertura do aplicativo para entrar no lugar do atual.
const RESTAURACAO_PENDENTE: &str = "restauracao-pendente.db";
/// Deixada pela troca: na abertura seguinte, todas as empresas passam a exigir a conferência da numeração.
const MARCA_RESTAURADO: &str = "restaurado.marca";

#[derive(Serialize)]
pub struct SituacaoBackup {
    pasta: String,
    /// Data e hora (UTC, RFC 3339) do último backup feito com sucesso.
    ultimo: Option<String>,
}

#[tauri::command]
pub async fn obter_backup(estado: State<'_, Estado>) -> R<SituacaoBackup> {
    exigir_cofre(&estado).await?;
    Ok(SituacaoBackup {
        pasta: estado
            .banco()?
            .configuracao(CHAVE_PASTA_BACKUP)
            .await
            .map_err(texto)?
            .unwrap_or_default(),
        ultimo: estado.banco()?.configuracao(CHAVE_ULTIMO_BACKUP).await.map_err(texto)?,
    })
}

#[tauri::command]
pub async fn definir_pasta_backup(estado: State<'_, Estado>, pasta: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let pasta = pasta.trim();
    if !pasta.is_empty() {
        std::fs::create_dir_all(pasta).map_err(|e| format!("não foi possível usar a pasta: {e}"))?;
    }
    estado
        .banco()?
        .definir_configuracao(CHAVE_PASTA_BACKUP, pasta)
        .await
        .map_err(texto)
}

/// Faz o backup agora na pasta configurada e devolve o nome do arquivo.
#[tauri::command]
pub async fn fazer_backup(estado: State<'_, Estado>) -> R<String> {
    executar(&estado).await
}

async fn executar(estado: &Estado) -> R<String> {
    let pasta = estado
        .banco()?
        .configuracao(CHAVE_PASTA_BACKUP)
        .await
        .map_err(texto)?
        .filter(|p| !p.trim().is_empty())
        .ok_or("defina a pasta do backup em Configurações")?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("cofre conferido acima");
    let arquivo = estado
        .banco()?
        .fazer_backup(emissor.cofre(), Path::new(&pasta))
        .await
        .map_err(texto)?;
    estado
        .banco()?
        .definir_configuracao(CHAVE_ULTIMO_BACKUP, &chrono::Utc::now().to_rfc3339())
        .await
        .map_err(texto)?;
    Ok(arquivo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default())
}

/// Backup automático ao abrir o cofre: só roda com pasta definida e último backup antigo.
/// Falha não impede o uso do sistema; fica na auditoria.
pub(crate) async fn automatico(estado: &Estado) {
    let Ok(banco) = estado.banco() else { return };
    let Ok(Some(pasta)) = banco.configuracao(CHAVE_PASTA_BACKUP).await else {
        return;
    };
    if pasta.trim().is_empty() {
        return;
    }
    let ultimo = banco
        .configuracao(CHAVE_ULTIMO_BACKUP)
        .await
        .ok()
        .flatten()
        .and_then(|u| chrono::DateTime::parse_from_rfc3339(&u).ok());
    if ultimo.is_some_and(|u| chrono::Utc::now() - u.to_utc() < INTERVALO_AUTOMATICO) {
        return;
    }
    if let Err(e) = executar(estado).await {
        let _ = banco.auditar("backup_falhou", None, Some(&e)).await;
    }
}

/// Decifra um backup para um arquivo novo e confere a integridade (serve de teste de
/// restauração). Não substitui o banco em uso: para restaurar de verdade, feche o sistema e
/// troque o `emissor.db` e o `emissor.cofre` pelos arquivos gerados.
#[tauri::command]
pub async fn restaurar_backup(estado: State<'_, Estado>, arquivo: String, senha: String, destino: String) -> R<()> {
    exigir_cofre(&estado).await?;
    let senha = zeroize::Zeroizing::new(senha);
    let destino = PathBuf::from(destino.trim());
    backup::restaurar_backup(Path::new(arquivo.trim()), &senha, &destino)
        .await
        .map_err(texto)
}

/// Restaura um backup no lugar do banco em uso: decifra e confere o backup na pasta de dados,
/// fecha o banco e reinicia o aplicativo; a troca dos arquivos acontece na abertura, antes de o
/// banco abrir ([`aplicar_restauracao_pendente`]). O banco atual fica guardado ao lado.
#[tauri::command]
pub async fn restaurar_e_reiniciar(
    app: tauri::AppHandle,
    estado: State<'_, Estado>,
    arquivo: String,
    senha: String,
) -> R<()> {
    exigir_cofre(&estado).await?;
    let senha = zeroize::Zeroizing::new(senha);
    let destino = estado.dados.join(RESTAURACAO_PENDENTE);
    // Sobra de uma tentativa interrompida: os dois arquivos são nossos e ainda não valem nada.
    for sobra in [destino.clone(), nfse_db::cofre::caminho_cabecalho(&destino)] {
        if sobra.exists() {
            std::fs::remove_file(&sobra).map_err(|e| format!("não foi possível preparar a restauração: {e}"))?;
        }
    }
    backup::restaurar_backup(Path::new(arquivo.trim()), &senha, &destino)
        .await
        .map_err(texto)?;
    // Como o "Bloquear": espera as operações em andamento e fecha o banco antes de reiniciar.
    *estado.emissor.write().await = None;
    if let Some(banco) = estado.trocar_banco(None) {
        banco.fechar().await;
    }
    app.restart()
}

/// `emissor.db` + `-wal` → `emissor.db-wal` (os arquivos auxiliares do SQLite vão junto do banco).
fn com_sufixo(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut nome = OsString::from(caminho.as_os_str());
    nome.push(sufixo);
    PathBuf::from(nome)
}

/// Na abertura do aplicativo, antes de o banco abrir: se há um banco restaurado completo
/// (`.db` e `.cofre`), o atual vai para `emissor-antes-da-restauracao-<data>.db` (com o `.cofre`,
/// o `-wal` e o `-shm`) e o restaurado entra no lugar. Se uma troca falhar, as anteriores são
/// desfeitas e o banco atual continua em uso.
pub(crate) fn aplicar_restauracao_pendente(dados: &Path) -> std::io::Result<bool> {
    let novo = dados.join(RESTAURACAO_PENDENTE);
    if !novo.exists() || !nfse_db::cofre::caminho_cabecalho(&novo).exists() {
        return Ok(false);
    }
    let atual = dados.join("emissor.db");
    let carimbo = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let guardado = dados.join(format!("emissor-antes-da-restauracao-{carimbo}.db"));
    let mut trocas = Vec::new();
    for (de, para) in [(&atual, &guardado), (&novo, &atual)] {
        for sufixo in ["", "-wal", "-shm"] {
            trocas.push((com_sufixo(de, sufixo), com_sufixo(para, sufixo)));
        }
        trocas.push((
            nfse_db::cofre::caminho_cabecalho(de),
            nfse_db::cofre::caminho_cabecalho(para),
        ));
    }
    let mut feitas = Vec::new();
    for (de, para) in trocas.into_iter().filter(|(de, _)| de.exists()) {
        if let Err(e) = std::fs::rename(&de, &para) {
            for (de, para) in feitas.iter().rev() {
                let _ = std::fs::rename(para, de);
            }
            return Err(e);
        }
        feitas.push((de, para));
    }
    std::fs::write(dados.join(MARCA_RESTAURADO), b"")?;
    Ok(true)
}

/// Primeira abertura depois de uma restauração: as notas emitidas depois do backup não estão no
/// banco, então a numeração de todas as empresas precisa ser conferida na Sefin antes de emitir.
pub(crate) async fn concluir_restauracao(estado: &Estado) -> R<Option<String>> {
    // Ainda pendente com o banco já aberto: a troca falhou na abertura e foi desfeita.
    let pendente = estado.dados.join(RESTAURACAO_PENDENTE);
    if pendente.exists() {
        for sobra in [nfse_db::cofre::caminho_cabecalho(&pendente), pendente] {
            let _ = std::fs::remove_file(sobra);
        }
        return Ok(Some(
            "A restauração do backup não pôde ser aplicada (algum arquivo da pasta de dados estava em uso); \
             o banco anterior continua em uso. Feche outros programas e restaure de novo em Configurações."
                .into(),
        ));
    }
    let marca = estado.dados.join(MARCA_RESTAURADO);
    if !marca.exists() {
        return Ok(None);
    }
    estado.banco()?.exigir_conferencia_de_numeracao().await.map_err(texto)?;
    std::fs::remove_file(&marca).map_err(texto)?;
    Ok(Some(
        "Backup restaurado. As notas emitidas depois dele não estão no sistema: antes de emitir, use \
         Empresas → Numeração / IM → Consultar na Sefin em cada empresa."
            .into(),
    ))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_temporaria(nome: &str) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!("emissor-teste-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    #[test]
    fn restauracao_pendente_troca_o_banco_e_guarda_o_anterior() {
        let dados = pasta_temporaria("restaurar");
        for (nome, conteudo) in [
            ("emissor.db", "atual"),
            ("emissor.db-wal", "wal atual"),
            ("emissor.cofre", "cofre atual"),
            (RESTAURACAO_PENDENTE, "restaurado"),
            ("restauracao-pendente.cofre", "cofre restaurado"),
        ] {
            std::fs::write(dados.join(nome), conteudo).unwrap();
        }
        assert!(aplicar_restauracao_pendente(&dados).unwrap());
        let ler = |nome: &str| std::fs::read_to_string(dados.join(nome)).unwrap();
        assert_eq!(ler("emissor.db"), "restaurado");
        assert_eq!(ler("emissor.cofre"), "cofre restaurado");
        // O -wal do banco antigo não pode ficar ao lado do restaurado.
        assert!(!dados.join("emissor.db-wal").exists());
        assert!(dados.join(MARCA_RESTAURADO).exists());
        let guardados: Vec<String> = std::fs::read_dir(&dados)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("emissor-antes-da-restauracao-"))
            .collect();
        assert_eq!(guardados.len(), 3, "{guardados:?}");
        assert!(!dados.join(RESTAURACAO_PENDENTE).exists());
        std::fs::remove_dir_all(&dados).unwrap();
    }

    #[test]
    fn restauracao_incompleta_nao_mexe_no_banco() {
        let dados = pasta_temporaria("incompleta");
        std::fs::write(dados.join("emissor.db"), "atual").unwrap();
        std::fs::write(dados.join(RESTAURACAO_PENDENTE), "sem cofre").unwrap();
        assert!(!aplicar_restauracao_pendente(&dados).unwrap());
        assert_eq!(std::fs::read_to_string(dados.join("emissor.db")).unwrap(), "atual");
        std::fs::remove_dir_all(&dados).unwrap();
    }
}
