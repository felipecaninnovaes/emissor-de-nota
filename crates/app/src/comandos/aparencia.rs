//! Aparência da interface. Lida antes da senha mestra (a tela de senha também usa o zoom), por isso
//! fica num arquivo ao lado do banco cifrado; não tem dado sigiloso.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use super::comum::{R, texto};
use crate::Estado;

const ZOOMS: [&str; 6] = ["0.9", "1", "1.1", "1.25", "1.4", "1.5"];
const DENSIDADES: [&str; 3] = ["auto", "confortavel", "compacta"];

#[derive(Serialize, Deserialize)]
pub struct Aparencia {
    /// Fator do tamanho do texto: "0.9", "1", "1.1", "1.25", "1.4" ou "1.5".
    zoom: String,
    /// "auto" (compacta em janelas baixas), "confortavel" ou "compacta".
    densidade: String,
}

fn arquivo(estado: &Estado) -> PathBuf {
    estado.dados.join("aparencia.json")
}

fn valida(a: &Aparencia) -> bool {
    ZOOMS.contains(&a.zoom.as_str()) && DENSIDADES.contains(&a.densidade.as_str())
}

#[tauri::command]
pub async fn obter_aparencia(estado: State<'_, Estado>) -> R<Aparencia> {
    let padrao = Aparencia {
        zoom: "1".into(),
        densidade: "auto".into(),
    };
    // Arquivo ausente ou alterado à mão volta ao padrão: aparência nunca impede o uso.
    Ok(std::fs::read(arquivo(&estado))
        .ok()
        .and_then(|b| serde_json::from_slice::<Aparencia>(&b).ok())
        .filter(valida)
        .unwrap_or(padrao))
}

#[tauri::command]
pub async fn salvar_aparencia(estado: State<'_, Estado>, aparencia: Aparencia) -> R<()> {
    if !valida(&aparencia) {
        return Err("opção de aparência inválida".into());
    }
    let json = serde_json::to_vec_pretty(&aparencia).map_err(texto)?;
    std::fs::write(arquivo(&estado), json).map_err(texto)
}
