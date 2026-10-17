//! Configuração do SMTP e envio da NFS-e por e-mail.

use serde::Serialize;
use tauri::State;

use super::comum::{R, emissor, exigir_cofre, texto};
use crate::Estado;

#[derive(Serialize)]
pub struct ConfigEmailDto {
    config: nfse_db::emails::ConfigSmtp,
    /// Já existe senha guardada (o campo da tela fica vazio para não mostrá-la).
    tem_senha: bool,
    configurado: bool,
}

#[tauri::command]
pub async fn obter_email(estado: State<'_, Estado>) -> R<ConfigEmailDto> {
    exigir_cofre(&estado).await?;
    let b = &estado.banco;
    let gravada = b.smtp().await.map_err(texto)?;
    Ok(ConfigEmailDto {
        configurado: gravada.is_some(),
        config: gravada.unwrap_or_else(|| nfse_db::emails::ConfigSmtp {
            servidor: String::new(),
            porta: 587,
            seguranca: nfse_db::emails::Seguranca::StartTls,
            usuario: None,
            remetente: String::new(),
            nome_remetente: None,
            assunto: nfse_db::emails::ConfigSmtp::ASSUNTO_PADRAO.into(),
            corpo: nfse_db::emails::ConfigSmtp::CORPO_PADRAO.into(),
            enviar_ao_emitir: false,
        }),
        tem_senha: b.tem_senha_smtp().await.map_err(texto)?,
    })
}

/// Grava a configuração; `senha` vazia mantém a que já estava guardada.
#[tauri::command]
pub async fn salvar_email(
    estado: State<'_, Estado>,
    config: nfse_db::emails::ConfigSmtp,
    senha: Option<String>,
) -> R<()> {
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    estado.banco.definir_smtp(&config).await.map_err(texto)?;
    if let Some(senha) = senha.filter(|s| !s.is_empty()) {
        estado.banco.definir_senha_smtp(cofre, &senha).await.map_err(texto)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn testar_email(estado: State<'_, Estado>, para: String) -> R<()> {
    let guarda = emissor!(estado);
    guarda
        .as_ref()
        .expect("verificado")
        .enviar_email_teste(para.trim(), &nfse_emissor::email::CarteiroSmtp)
        .await
        .map_err(texto)
}

/// Destinatários padrão da nota (e-mails de envio do tomador ou o e-mail do cadastro).
#[tauri::command]
pub async fn destinatarios_email(estado: State<'_, Estado>, nfse_id: i64) -> R<Vec<String>> {
    exigir_cofre(&estado).await?;
    Ok(estado
        .banco
        .dados_email_nfse(nfse_id)
        .await
        .map_err(texto)?
        .destinatarios)
}

#[tauri::command]
pub async fn enviar_email(estado: State<'_, Estado>, nfse_id: i64, para: String) -> R<String> {
    let para = nfse_db::emails::separar_emails(&para);
    let guarda = emissor!(estado);
    let enviados = guarda
        .as_ref()
        .expect("verificado")
        .enviar_email(nfse_id, &para, &nfse_emissor::email::CarteiroSmtp)
        .await
        .map_err(texto)?;
    Ok(format!("E-mail enviado para {}.", enviados.join(", ")))
}
