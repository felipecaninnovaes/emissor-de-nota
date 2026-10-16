//! Fontes do DANFSe (NT 008/2026, item 2.4): Arial nos títulos e Microsoft Sans Serif nos conteúdos.
//!
//! As fontes da Microsoft não podem ir para o repositório: são lidas da pasta de fontes do
//! Windows. Fora do Windows (CI), usa a Liberation Sans, que tem as mesmas métricas da Arial.

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum ErroFonte {
    #[error("fonte {0} não encontrada (procurada em: {1})")]
    NaoEncontrada(&'static str, String),
    #[error("arquivo de fonte inválido: {0}")]
    Invalida(String),
}

#[derive(Clone)]
pub struct Fonte {
    pub(crate) krilla: krilla::text::Font,
    dados: Arc<Vec<u8>>,
}

impl Fonte {
    pub fn carregar(caminho: &Path) -> Result<Self, ErroFonte> {
        let dados =
            Arc::new(std::fs::read(caminho).map_err(|e| ErroFonte::Invalida(format!("{}: {e}", caminho.display())))?);
        ttf_parser::Face::parse(&dados, 0).map_err(|e| ErroFonte::Invalida(format!("{}: {e}", caminho.display())))?;
        let krilla = krilla::text::Font::new(dados.clone().into(), 0)
            .ok_or_else(|| ErroFonte::Invalida(caminho.display().to_string()))?;
        Ok(Self { krilla, dados })
    }

    /// Largura do texto em pontos.
    pub fn largura(&self, texto: &str, tamanho: f32) -> f32 {
        let face = ttf_parser::Face::parse(&self.dados, 0).expect("validada ao carregar");
        let upem = f32::from(face.units_per_em());
        let unidades: f32 = texto
            .chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|g| face.glyph_hor_advance(g))
                    .map_or(upem / 2.0, f32::from)
            })
            .sum();
        unidades * tamanho / upem
    }
}

#[derive(Clone)]
pub struct Fontes {
    pub titulo: Fonte,
    pub titulo_negrito: Fonte,
    pub conteudo: Fonte,
}

fn primeira_existente(nome: &'static str, candidatos: &[PathBuf]) -> Result<Fonte, ErroFonte> {
    match candidatos.iter().find(|c| c.is_file()) {
        Some(c) => Fonte::carregar(c),
        None => Err(ErroFonte::NaoEncontrada(
            nome,
            candidatos
                .iter()
                .map(|c| c.display().to_string())
                .collect::<Vec<_>>()
                .join("; "),
        )),
    }
}

impl Fontes {
    pub fn do_sistema() -> Result<Self, ErroFonte> {
        let windows = std::env::var_os("WINDIR")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("Fonts");
        let liberation = [
            "/usr/share/fonts/truetype/liberation",
            "/usr/share/fonts/truetype/liberation2",
            "/usr/share/fonts/liberation",
        ];
        let opcoes = |arquivo_windows: &str, arquivo_liberation: &str| -> Vec<PathBuf> {
            std::iter::once(windows.join(arquivo_windows))
                .chain(liberation.iter().map(|d| Path::new(d).join(arquivo_liberation)))
                .collect()
        };
        let titulo = primeira_existente("Arial", &opcoes("arial.ttf", "LiberationSans-Regular.ttf"))?;
        let titulo_negrito = primeira_existente("Arial Negrito", &opcoes("arialbd.ttf", "LiberationSans-Bold.ttf"))?;
        let conteudo = primeira_existente(
            "Microsoft Sans Serif",
            &opcoes("micross.ttf", "LiberationSans-Regular.ttf"),
        )?;
        Ok(Self {
            titulo,
            titulo_negrito,
            conteudo,
        })
    }
}
