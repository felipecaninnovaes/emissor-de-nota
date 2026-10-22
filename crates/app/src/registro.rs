//! Log do aplicativo em arquivo (`<dados>/logs/emissor.AAAA-MM-DD.log`), um por dia, guardando os
//! últimos [`DIAS_GUARDADOS`]. Registra o ciclo da emissão, a resposta da Sefin e erros, nunca senha,
//! chave privada nem o XML completo (para isso há o diagnóstico da nota).
//!
//! O modo detalhado (`debug`) vem desligado e é lembrado por um arquivo-marca na pasta de dados, porque
//! o log sobe antes de o banco abrir.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Registry, reload};

pub const DIAS_GUARDADOS: usize = 14;

const PREFIXO: &str = "emissor";
const MARCA_DETALHADO: &str = "log-detalhado";
/// Quanto do fim do arquivo é lido ao mostrar as últimas linhas.
const LIMITE_LEITURA: u64 = 256 * 1024;

static FILTRO: OnceLock<reload::Handle<Targets, Registry>> = OnceLock::new();
static GUARDA: OnceLock<WorkerGuard> = OnceLock::new();

pub fn pasta_logs(dados: &Path) -> PathBuf {
    dados.join("logs")
}

/// Só os crates do emissor falam no nível pedido; as bibliotecas (rede, banco) ficam em aviso para o
/// modo detalhado não despejar detalhes de terceiros.
fn filtro(detalhado: bool) -> Targets {
    let nivel = if detalhado {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    Targets::new()
        .with_default(LevelFilter::WARN)
        .with_target("emissor_nfse", nivel)
        .with_target("nfse_emissor", nivel)
        .with_target("nfse_nacional", nivel)
        .with_target("nfse_xml", nivel)
        .with_target("nfse_db", nivel)
}

/// Liga o log em arquivo e o registro de pânicos. Falhar aqui não impede o aplicativo de abrir.
pub fn iniciar(dados: &Path) {
    if FILTRO.get().is_some() {
        return;
    }
    let pasta = pasta_logs(dados);
    let arquivo = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(PREFIXO)
        .filename_suffix("log")
        .max_log_files(DIAS_GUARDADOS)
        .build(&pasta);
    let Ok(arquivo) = arquivo else { return };
    let (escritor, guarda) = tracing_appender::non_blocking(arquivo);
    let (camada, alca) = reload::Layer::new(filtro(detalhado(dados)));
    let saida = tracing_subscriber::fmt::layer()
        .with_writer(escritor)
        .with_ansi(false)
        .with_target(true)
        .with_filter(camada);
    if tracing_subscriber::registry().with(saida).try_init().is_err() {
        return;
    }
    let _ = FILTRO.set(alca);
    let _ = GUARDA.set(guarda);
    registrar_panicos();
    tracing::info!(
        versao = env!("CARGO_PKG_VERSION"),
        detalhado = detalhado(dados),
        "aplicativo iniciado"
    );
}

fn registrar_panicos() {
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let local = info.location().map(|l| format!("{}:{}", l.file(), l.line()));
        let mensagem = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        tracing::error!(local = local.as_deref().unwrap_or("?"), %mensagem, "pânico");
        anterior(info);
    }));
}

pub fn detalhado(dados: &Path) -> bool {
    dados.join(MARCA_DETALHADO).exists()
}

/// Liga ou desliga o modo detalhado na hora e lembra a escolha para a próxima abertura.
pub fn definir_detalhado(dados: &Path, ligado: bool) -> Result<(), String> {
    let marca = dados.join(MARCA_DETALHADO);
    if ligado {
        std::fs::write(&marca, "").map_err(|e| format!("{}: {e}", marca.display()))?;
    } else if marca.exists() {
        std::fs::remove_file(&marca).map_err(|e| format!("{}: {e}", marca.display()))?;
    }
    if let Some(alca) = FILTRO.get() {
        let _ = alca.reload(filtro(ligado));
    }
    tracing::info!(detalhado = ligado, "modo detalhado do log alterado");
    Ok(())
}

/// Últimas `n` linhas do log mais recente (vazio se ainda não há log).
pub fn ultimas_linhas(dados: &Path, n: usize) -> String {
    let pasta = pasta_logs(dados);
    let Ok(entradas) = std::fs::read_dir(&pasta) else {
        return String::new();
    };
    // O nome leva a data, então o maior nome é o dia mais recente.
    let Some(recente) = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.starts_with(PREFIXO) && f.ends_with(".log"))
        })
        .max()
    else {
        return String::new();
    };
    let Ok(mut arquivo) = std::fs::File::open(&recente) else {
        return String::new();
    };
    let tamanho = arquivo.metadata().map(|m| m.len()).unwrap_or(0);
    let inicio = tamanho.saturating_sub(LIMITE_LEITURA);
    let mut bytes = Vec::new();
    if arquivo.seek(SeekFrom::Start(inicio)).is_err() || arquivo.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let texto = String::from_utf8_lossy(&bytes);
    let mut linhas: Vec<&str> = texto.lines().collect();
    if inicio > 0 && !linhas.is_empty() {
        // A primeira linha lida pode estar cortada no meio.
        linhas.remove(0);
    }
    let corte = linhas.len().saturating_sub(n);
    linhas[corte..].join("\n")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_temporaria(nome: &str) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!("emissor-registro-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    #[test]
    fn modo_detalhado_e_lembrado_na_pasta_de_dados() {
        let dados = pasta_temporaria("detalhado");
        assert!(!detalhado(&dados));
        definir_detalhado(&dados, true).unwrap();
        assert!(detalhado(&dados));
        definir_detalhado(&dados, false).unwrap();
        assert!(!detalhado(&dados));
        let _ = std::fs::remove_dir_all(&dados);
    }

    #[test]
    fn mostra_as_ultimas_linhas_do_log_mais_recente() {
        let dados = pasta_temporaria("linhas");
        assert_eq!(ultimas_linhas(&dados, 5), "");
        let logs = pasta_logs(&dados);
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("emissor.2026-10-21.log"), "antigo\n").unwrap();
        let recente: String = (1..=10).map(|i| format!("linha {i}\n")).collect();
        std::fs::write(logs.join("emissor.2026-10-22.log"), recente).unwrap();
        std::fs::write(logs.join("outro.txt"), "ignorado\n").unwrap();
        assert_eq!(ultimas_linhas(&dados, 3), "linha 8\nlinha 9\nlinha 10");
        assert_eq!(ultimas_linhas(&dados, 50).lines().count(), 10);
        let _ = std::fs::remove_dir_all(&dados);
    }

    #[test]
    fn filtro_so_abre_o_detalhe_para_os_crates_do_emissor() {
        let normal = filtro(false);
        assert!(normal.would_enable("nfse_emissor::lib", &tracing::Level::INFO));
        assert!(!normal.would_enable("nfse_emissor::lib", &tracing::Level::DEBUG));
        let detalhado = filtro(true);
        assert!(detalhado.would_enable("nfse_nacional", &tracing::Level::DEBUG));
        assert!(!detalhado.would_enable("hyper::proto", &tracing::Level::DEBUG));
        assert!(detalhado.would_enable("hyper::proto", &tracing::Level::WARN));
    }
}
