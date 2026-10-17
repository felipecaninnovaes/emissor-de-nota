//! Aparência da interface (lida antes da senha mestra; não tem dado sigiloso).

use serde::{Deserialize, Serialize};
use tauri::State;

use super::comum::{R, texto};
use crate::Estado;

const CHAVE_ZOOM: &str = "interface_zoom";
const CHAVE_DENSIDADE: &str = "interface_densidade";
const ZOOMS: [&str; 6] = ["0.9", "1", "1.1", "1.25", "1.4", "1.5"];
const DENSIDADES: [&str; 3] = ["auto", "confortavel", "compacta"];

#[derive(Serialize, Deserialize)]
pub struct Aparencia {
    /// Fator do tamanho do texto: "0.9", "1", "1.1", "1.25", "1.4" ou "1.5".
    zoom: String,
    /// "auto" (compacta em janelas baixas), "confortavel" ou "compacta".
    densidade: String,
}

#[tauri::command]
pub async fn obter_aparencia(estado: State<'_, Estado>) -> R<Aparencia> {
    let ler = |chave| estado.banco.configuracao(chave);
    let zoom = ler(CHAVE_ZOOM)
        .await
        .map_err(texto)?
        .filter(|z| ZOOMS.contains(&z.as_str()));
    let densidade = ler(CHAVE_DENSIDADE)
        .await
        .map_err(texto)?
        .filter(|d| DENSIDADES.contains(&d.as_str()));
    Ok(Aparencia {
        zoom: zoom.unwrap_or_else(|| "1".into()),
        densidade: densidade.unwrap_or_else(|| "auto".into()),
    })
}

#[tauri::command]
pub async fn salvar_aparencia(estado: State<'_, Estado>, aparencia: Aparencia) -> R<()> {
    if !ZOOMS.contains(&aparencia.zoom.as_str()) || !DENSIDADES.contains(&aparencia.densidade.as_str()) {
        return Err("opção de aparência inválida".into());
    }
    estado
        .banco
        .definir_configuracao(CHAVE_ZOOM, &aparencia.zoom)
        .await
        .map_err(texto)?;
    estado
        .banco
        .definir_configuracao(CHAVE_DENSIDADE, &aparencia.densidade)
        .await
        .map_err(texto)
}
