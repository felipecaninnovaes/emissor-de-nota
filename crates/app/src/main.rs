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
            comandos::cofre::situacao_cofre,
            comandos::cofre::criar_cofre,
            comandos::cofre::abrir_cofre,
            comandos::cofre::bloquear_cofre,
            comandos::cofre::obter_pasta,
            comandos::cofre::definir_pasta,
            comandos::aparencia::obter_aparencia,
            comandos::aparencia::salvar_aparencia,
            comandos::empresas::listar_empresas,
            comandos::empresas::cadastrar_empresa,
            comandos::empresas::atualizar_empresa,
            comandos::empresas::importar_certificado,
            comandos::empresas::excluir_empresa,
            comandos::empresas::definir_im_na_dps,
            comandos::empresas::definir_proximo_numero,
            comandos::empresas::ajustar_numeracao,
            comandos::empresas::certificados_vencendo,
            comandos::tomadores::buscar_tomadores,
            comandos::tomadores::salvar_tomador,
            comandos::servicos::listar_servicos,
            comandos::servicos::salvar_servico,
            comandos::servicos::ler_planilha_servicos,
            comandos::servicos::importar_servicos,
            comandos::servicos::salvar_modelo_servicos,
            comandos::servicos::exportar_servicos,
            comandos::servicos::desativar_servico,
            comandos::emissao::previa_dps,
            comandos::emissao::emitir,
            comandos::notas::listar_notas,
            comandos::notas::cancelar,
            comandos::notas::reconciliar,
            comandos::notas::dados_para_refazer,
            comandos::notas::pendentes,
            comandos::notas::abrir_arquivo,
            comandos::notas::abrir_nota,
            comandos::recorrencias::listar_recorrencias,
            comandos::recorrencias::salvar_recorrencia,
            comandos::recorrencias::desativar_recorrencia,
            comandos::recorrencias::emitir_recorrencia,
            comandos::planilha::ler_planilha,
            comandos::planilha::emitir_planilha,
            comandos::planilha::salvar_modelo_planilha,
            comandos::email::obter_email,
            comandos::email::salvar_email,
            comandos::email::testar_email,
            comandos::email::destinatarios_email,
            comandos::email::enviar_email,
            comandos::consultas::consultar_cnpj,
            comandos::consultas::consultar_cep,
            comandos::atualizacao::versao_app,
            comandos::atualizacao::procurar_atualizacao,
            comandos::atualizacao::instalar_atualizacao,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
