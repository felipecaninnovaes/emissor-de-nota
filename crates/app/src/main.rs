//! Aplicativo desktop (Tauri 2). A interface fica em `ui/` (HTML/CSS/JS sem etapa de build) e
//! chama os comandos abaixo; toda regra fiscal está nos crates `nfse-*`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cep;
mod cnpj;
mod comandos;
mod pasta_dados;

use std::path::PathBuf;

use tauri::Manager;
use tokio::sync::RwLock;

use nfse_db::Banco;
use nfse_emissor::{ConectorSefin, Emissor};

pub struct Estado {
    /// Pasta de dados do aplicativo: banco cifrado, cabeçalho do cofre e aparência.
    pub dados: PathBuf,
    /// O banco só abre com a senha mestra (SQLCipher); é `None` até lá e volta a `None` ao bloquear.
    /// Lock síncrono, segurado só para clonar: nunca atravessa um `await`.
    banco: std::sync::RwLock<Option<Banco>>,
    /// Preenchido depois que o operador abre o cofre com a senha mestra.
    pub emissor: RwLock<Option<Emissor<ConectorSefin>>>,
}

impl Estado {
    pub fn caminho_banco(&self) -> PathBuf {
        self.dados.join("emissor.db")
    }

    /// O banco aberto, ou o aviso padrão de que falta a senha mestra.
    pub fn banco(&self) -> Result<Banco, String> {
        self.banco
            .read()
            .ok()
            .and_then(|b| b.clone())
            .ok_or_else(|| "abra o sistema com a senha mestra".to_string())
    }

    /// Troca o banco aberto e devolve o anterior (para fechá-lo).
    pub fn trocar_banco(&self, novo: Option<Banco>) -> Option<Banco> {
        self.banco
            .write()
            .ok()
            .and_then(|mut b| std::mem::replace(&mut *b, novo))
    }
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

/// No KDE com Wayland, o GTK3 desenha a própria barra de título (não implementa `xdg-decoration`)
/// e destoa do Breeze. Pelo XWayland, quem desenha a barra é o KWin. Só vale quando há XWayland
/// (`DISPLAY`) e o operador não escolheu um `GDK_BACKEND`.
#[cfg(target_os = "linux")]
fn preferir_x11_no_kde() {
    use std::env::var_os;
    let kde = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.split(':').any(|p| p == "KDE"));
    if kde && var_os("WAYLAND_DISPLAY").is_some() && var_os("DISPLAY").is_some() && var_os("GDK_BACKEND").is_none() {
        // SAFETY: chamado no início do `main`, antes de qualquer outra thread existir.
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }
}

fn main() {
    use tauri_plugin_window_state::StateFlags;

    #[cfg(target_os = "linux")]
    preferir_x11_no_kde();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Antes de qualquer leitura da pasta: no macOS ela muda de nome e é migrada aqui. A
            // posição da janela (`window-state`) mora na mesma pasta e vai junto.
            let dados = pasta_dados::resolver(app.path().app_data_dir()?)?;
            dimensionar_primeira_abertura(app)?;
            std::fs::create_dir_all(&dados)?;
            app.manage(Estado {
                dados,
                banco: std::sync::RwLock::new(None),
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
            comandos::dialogo::escolher_pasta,
            comandos::dialogo::escolher_arquivo,
            comandos::dialogo::ler_arquivo_escolhido,
            comandos::dialogo::salvar_arquivo,
            comandos::backup::obter_backup,
            comandos::backup::definir_pasta_backup,
            comandos::backup::fazer_backup,
            comandos::backup::restaurar_backup,
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
            comandos::empresas::exportar_empresas,
            comandos::empresas::ler_exportacao_empresas,
            comandos::empresas::importar_empresas,
            comandos::tomadores::buscar_tomadores,
            comandos::tomadores::buscar_tomadores_de_outras_empresas,
            comandos::tomadores::vincular_tomador,
            comandos::tomadores::salvar_tomador,
            comandos::servicos::listar_servicos,
            comandos::servicos::salvar_servico,
            comandos::servicos::buscar_lista_servicos,
            comandos::servicos::buscar_nbs,
            comandos::servicos::buscar_class_trib,
            comandos::servicos::buscar_ind_operacao,
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
            comandos::notas::sincronizar_cancelamento,
            comandos::notas::dados_para_refazer,
            comandos::notas::dados_para_copiar,
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
