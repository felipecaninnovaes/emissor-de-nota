//! Pasta de dados do aplicativo. No macOS o nome padrão do Tauri (`br.emissor-nfse.app`) termina
//! em `.app`, e o Finder passa a tratar a pasta como pacote de aplicativo. Lá usamos o nome sem o
//! sufixo e movemos para ele, na primeira execução, o que já existia na pasta antiga
//! ([ADR 0014](../../../docs/decisoes/0014-pasta-de-dados-no-macos.md)).

use std::io;
#[cfg(any(target_os = "macos", test))]
use std::path::Path;
use std::path::PathBuf;

/// Nome da pasta de dados no macOS, sem o sufixo `.app`.
#[cfg(any(target_os = "macos", test))]
const NOME_MACOS: &str = "br.emissor-nfse";

/// Pasta de dados a usar, a partir da que o Tauri sugere (`app_data_dir()`).
pub fn resolver(padrao: PathBuf) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let nova = padrao.with_file_name(NOME_MACOS);
        migrar(&padrao, &nova)?;
        Ok(nova)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(padrao)
    }
}

/// Move o conteúdo de `antiga` para `nova` sem sobrescrever nada, arquivo a arquivo (cada
/// `rename` é atômico; se for interrompida, a próxima execução completa). O arquivo do
/// `window-state` fica onde está: o plugin o lê e grava em `app_config_dir()`, que no macOS é a
/// pasta antiga, e movê-lo faria a janela esquecer o tamanho e a posição.
#[cfg(any(target_os = "macos", test))]
fn migrar(antiga: &Path, nova: &Path) -> io::Result<()> {
    if !antiga.is_dir() || antiga == nova {
        return Ok(());
    }
    std::fs::create_dir_all(nova)?;
    for entrada in std::fs::read_dir(antiga)? {
        let origem = entrada?.path();
        let Some(nome) = origem.file_name() else {
            continue;
        };
        if nome == tauri_plugin_window_state::DEFAULT_FILENAME {
            continue;
        }
        let destino = nova.join(nome);
        if !destino.exists() {
            std::fs::rename(&origem, &destino)?;
        }
    }
    // Só remove a pasta antiga se ficou vazia; o que restou (conflito) fica para o operador.
    let _ = std::fs::remove_dir(antiga);
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn pasta_temporaria() -> PathBuf {
        static CONTADOR: AtomicU32 = AtomicU32::new(0);
        let pasta = std::env::temp_dir().join(format!(
            "emissor-pasta-dados-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    fn par(base: &Path) -> (PathBuf, PathBuf) {
        (base.join("br.emissor-nfse.app"), base.join(NOME_MACOS))
    }

    #[test]
    fn move_a_pasta_inteira_quando_a_nova_nao_existe() {
        let base = pasta_temporaria();
        let (antiga, nova) = par(&base);
        std::fs::create_dir_all(antiga.join("backups")).unwrap();
        std::fs::write(antiga.join("emissor.db"), b"banco").unwrap();
        std::fs::write(antiga.join("emissor.cofre"), b"cofre").unwrap();
        std::fs::write(antiga.join("backups/a.nfbk"), b"bkp").unwrap();

        migrar(&antiga, &nova).unwrap();

        assert!(!antiga.exists());
        assert_eq!(std::fs::read(nova.join("emissor.db")).unwrap(), b"banco");
        assert_eq!(std::fs::read(nova.join("emissor.cofre")).unwrap(), b"cofre");
        assert_eq!(std::fs::read(nova.join("backups/a.nfbk")).unwrap(), b"bkp");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn nao_sobrescreve_o_que_ja_existe_na_nova() {
        let base = pasta_temporaria();
        let (antiga, nova) = par(&base);
        std::fs::create_dir_all(&antiga).unwrap();
        std::fs::create_dir_all(&nova).unwrap();
        std::fs::write(antiga.join("emissor.db"), b"antigo").unwrap();
        std::fs::write(antiga.join("aparencia.json"), b"{}").unwrap();
        std::fs::write(nova.join("emissor.db"), b"novo").unwrap();

        migrar(&antiga, &nova).unwrap();

        assert_eq!(std::fs::read(nova.join("emissor.db")).unwrap(), b"novo");
        assert_eq!(std::fs::read(nova.join("aparencia.json")).unwrap(), b"{}");
        // O arquivo em conflito não é apagado.
        assert_eq!(std::fs::read(antiga.join("emissor.db")).unwrap(), b"antigo");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn mantem_o_arquivo_do_window_state_na_pasta_antiga() {
        let base = pasta_temporaria();
        let (antiga, nova) = par(&base);
        std::fs::create_dir_all(&antiga).unwrap();
        std::fs::write(antiga.join("emissor.db"), b"banco").unwrap();
        let estado = antiga.join(tauri_plugin_window_state::DEFAULT_FILENAME);
        std::fs::write(&estado, b"{}").unwrap();

        migrar(&antiga, &nova).unwrap();

        assert!(estado.exists());
        assert!(!nova.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists());
        assert!(nova.join("emissor.db").exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn sem_pasta_antiga_nao_faz_nada() {
        let base = pasta_temporaria();
        let (antiga, nova) = par(&base);

        migrar(&antiga, &nova).unwrap();

        assert!(!antiga.exists() && !nova.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn migracao_repetida_e_inofensiva() {
        let base = pasta_temporaria();
        let (antiga, nova) = par(&base);
        std::fs::create_dir_all(&antiga).unwrap();
        std::fs::write(antiga.join("emissor.db"), b"banco").unwrap();

        migrar(&antiga, &nova).unwrap();
        migrar(&antiga, &nova).unwrap();

        assert_eq!(std::fs::read(nova.join("emissor.db")).unwrap(), b"banco");
        std::fs::remove_dir_all(base).unwrap();
    }
}
