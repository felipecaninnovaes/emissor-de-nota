//! Aplicativo desktop (Tauri 2). A interface fica em `ui/` (HTML/CSS/JS sem etapa de build) e
//! chama os comandos abaixo; toda regra fiscal está nos crates `nfse-*`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
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
            comandos::obter_pasta,
            comandos::definir_pasta,
            comandos::listar_empresas,
            comandos::cadastrar_empresa,
            comandos::importar_certificado,
            comandos::definir_im_na_dps,
            comandos::definir_proximo_numero,
            comandos::certificados_vencendo,
            comandos::buscar_tomadores,
            comandos::salvar_tomador,
            comandos::listar_servicos,
            comandos::salvar_servico,
            comandos::desativar_servico,
            comandos::previa_dps,
            comandos::emitir,
            comandos::listar_notas,
            comandos::cancelar,
            comandos::reconciliar,
            comandos::pendentes,
            comandos::abrir_arquivo,
            comandos::abrir_nota,
            comandos::consultar_cnpj,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
