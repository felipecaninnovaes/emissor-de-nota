//! Aplicativo desktop (Tauri 2). A interface fica em `ui/` (HTML/CSS/JS sem etapa de build) e
//! chama os comandos abaixo; toda regra fiscal está nos crates `nfse-*`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cep;
mod cnpj;
mod comandos;

use std::path::PathBuf;

use tauri::Manager;
use tokio::sync::RwLock;

use nfse_db::Banco;
use nfse_emissor::{ConectorSefin, Emissor};

pub struct Estado {
    pub banco: Banco,
    /// Preenchido depois que o operador abre o cofre com a senha mestra.
    pub emissor: RwLock<Option<Emissor<ConectorSefin>>>,
}

/// Na primeira abertura (sem tamanho guardado pelo plugin window-state) a janela ocupa 90% da
/// área útil do monitor, centralizada; depois volta sempre com o tamanho e a posição da última vez.
fn dimensionar_primeira_abertura(app: &tauri::App) -> tauri::Result<()> {
    let guardado = app
        .path()
        .app_config_dir()?
        .join(tauri_plugin_window_state::DEFAULT_FILENAME);
    if guardado.exists() {
        return Ok(());
    }
    let Some(janela) = app.get_webview_window("main") else {
        return Ok(());
    };
    if let Some(monitor) = janela.current_monitor()? {
        let area = monitor.work_area().size;
        janela.set_size(tauri::PhysicalSize::new(area.width * 9 / 10, area.height * 9 / 10))?;
        janela.center()?;
    }
    Ok(())
}

fn main() {
    use tauri_plugin_window_state::StateFlags;

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            dimensionar_primeira_abertura(app)?;
            let dados: PathBuf = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dados)?;
            let banco = tauri::async_runtime::block_on(Banco::abrir(&dados.join("emissor.db")))?;
            app.manage(Estado {
                banco,
                emissor: RwLock::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::situacao_cofre,
            comandos::criar_cofre,
            comandos::abrir_cofre,
            comandos::bloquear_cofre,
            comandos::obter_pasta,
            comandos::definir_pasta,
            comandos::obter_aparencia,
            comandos::salvar_aparencia,
            comandos::listar_empresas,
            comandos::cadastrar_empresa,
            comandos::atualizar_empresa,
            comandos::importar_certificado,
            comandos::excluir_empresa,
            comandos::definir_im_na_dps,
            comandos::definir_proximo_numero,
            comandos::ajustar_numeracao,
            comandos::certificados_vencendo,
            comandos::buscar_tomadores,
            comandos::salvar_tomador,
            comandos::listar_servicos,
            comandos::salvar_servico,
            comandos::ler_planilha_servicos,
            comandos::importar_servicos,
            comandos::salvar_modelo_servicos,
            comandos::exportar_servicos,
            comandos::desativar_servico,
            comandos::previa_dps,
            comandos::emitir,
            comandos::listar_notas,
            comandos::cancelar,
            comandos::reconciliar,
            comandos::dados_para_refazer,
            comandos::pendentes,
            comandos::abrir_arquivo,
            comandos::abrir_nota,
            comandos::listar_recorrencias,
            comandos::salvar_recorrencia,
            comandos::desativar_recorrencia,
            comandos::emitir_recorrencia,
            comandos::ler_planilha,
            comandos::emitir_planilha,
            comandos::salvar_modelo_planilha,
            comandos::email::obter_email,
            comandos::email::salvar_email,
            comandos::email::testar_email,
            comandos::email::destinatarios_email,
            comandos::email::enviar_email,
            comandos::consultar_cnpj,
            comandos::consultar_cep,
            comandos::atualizacao::versao_app,
            comandos::atualizacao::procurar_atualizacao,
            comandos::atualizacao::instalar_atualizacao,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
