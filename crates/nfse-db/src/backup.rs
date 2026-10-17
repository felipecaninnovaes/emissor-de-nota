//! Backup cifrado do banco (`.nfbk`).
//!
//! O banco já guarda os XMLs autorizados e os eventos, então a cópia dele cobre a guarda de 5
//! anos. O arquivo é cifrado com a mesma chave do cofre (AES-256-GCM, chave derivada da senha
//! mestra): o cabeçalho leva só o sal e os parâmetros do Argon2 (nenhum segredo), e a senha
//! mestra basta para abrir o backup em outra máquina. Perder a senha mestra perde o backup, como
//! já perde os certificados.
//!
//! Formato: `NFBK1` | u32 tamanho do cabeçalho | cabeçalho JSON ([`CabecalhoCofre`]) | blocos
//! (u32 tamanho | nonce + texto cifrado) de até 1 MiB. Cada bloco amarra no AAD o índice e se é
//! o último, para detectar bloco removido, repetido, reordenado ou arquivo truncado.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::abertura::conferir_integridade;
use crate::cofre::{CabecalhoCofre, Cofre, gravar_cabecalho};
use crate::{Banco, ErroBanco, Resultado, json};

const MAGICO: &[u8] = b"NFBK1";
const TAM_BLOCO: usize = 1024 * 1024;
/// Teto de um bloco cifrado lido do arquivo (bloco + nonce + tag, com folga), contra arquivo forjado.
const TETO_BLOCO_CIFRADO: usize = TAM_BLOCO + 64;
const TETO_CABECALHO: usize = 4096;
const PREFIXO: &str = "emissor-";
pub const EXTENSAO: &str = "nfbk";
/// Quantos backups ficam na pasta; os mais antigos são apagados depois de um backup novo.
pub const COPIAS_MANTIDAS: usize = 10;

fn contexto(indice: u64, ultimo: bool) -> String {
    format!("backup:v1:{indice}:{}", u8::from(ultimo))
}

fn erro_io(e: std::io::Error) -> ErroBanco {
    ErroBanco::Regra(format!("falha de arquivo no backup: {e}"))
}

/// Apaga o arquivo ao sair de escopo (cópia do banco em claro e arquivos parciais).
struct ApagarAoSair(PathBuf);

impl ApagarAoSair {
    /// O arquivo deu certo: não apagar.
    fn manter(self) {
        std::mem::forget(self);
    }
}

impl Drop for ApagarAoSair {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Banco {
    /// Grava em `pasta` um backup cifrado do banco e devolve o caminho do arquivo. Mantém só as
    /// [`COPIAS_MANTIDAS`] mais recentes.
    pub async fn fazer_backup(&self, cofre: &Cofre, pasta: &Path) -> Resultado<PathBuf> {
        let cabecalho = cofre.cabecalho();
        std::fs::create_dir_all(pasta).map_err(erro_io)?;

        // Cópia consistente do banco (inclui o que ainda está no WAL). Fica em claro só no
        // diretório temporário do sistema, nunca na pasta de destino (que pode ser sincronizada).
        static SEQUENCIA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unico = format!(
            "{}-{}",
            std::process::id(),
            SEQUENCIA.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let copia = ApagarAoSair(std::env::temp_dir().join(format!("nfse-backup-{unico}.db")));
        sqlx::query("VACUUM INTO ?")
            .bind(copia.0.to_string_lossy().into_owned())
            .execute(self.pool())
            .await?;

        let nome = format!("{PREFIXO}{}.{EXTENSAO}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let destino = pasta.join(nome);
        let parcial = ApagarAoSair(pasta.join(format!(".{unico}.parcial")));
        cifrar_arquivo(cofre, cabecalho, &copia.0, &parcial.0)?;
        std::fs::rename(&parcial.0, &destino).map_err(erro_io)?;

        podar(pasta, COPIAS_MANTIDAS);
        self.auditar("backup_criado", None, destino.file_name().and_then(|n| n.to_str()))
            .await?;
        Ok(destino)
    }
}

fn cifrar_arquivo(cofre: &Cofre, cabecalho: &CabecalhoCofre, origem: &Path, saida: &Path) -> Resultado<()> {
    let tamanho = std::fs::metadata(origem).map_err(erro_io)?.len();
    let blocos = tamanho.div_ceil(TAM_BLOCO as u64).max(1);
    let mut entrada = std::fs::File::open(origem).map_err(erro_io)?;
    let mut arquivo = std::fs::File::create(saida).map_err(erro_io)?;

    let cab = json(cabecalho).into_bytes();
    arquivo.write_all(MAGICO).map_err(erro_io)?;
    arquivo.write_all(&(cab.len() as u32).to_be_bytes()).map_err(erro_io)?;
    arquivo.write_all(&cab).map_err(erro_io)?;

    let mut buffer = vec![0u8; TAM_BLOCO];
    for indice in 0..blocos {
        let esperado = if indice + 1 == blocos {
            (tamanho - indice * TAM_BLOCO as u64) as usize
        } else {
            TAM_BLOCO
        };
        entrada.read_exact(&mut buffer[..esperado]).map_err(erro_io)?;
        let cifrado = cofre.cifrar(&buffer[..esperado], &contexto(indice, indice + 1 == blocos))?;
        arquivo
            .write_all(&(cifrado.len() as u32).to_be_bytes())
            .map_err(erro_io)?;
        arquivo.write_all(&cifrado).map_err(erro_io)?;
    }
    arquivo.sync_all().map_err(erro_io)
}

/// Apaga os backups mais antigos além de `manter`. Só mexe em `emissor-*.nfbk` da própria pasta.
fn podar(pasta: &Path, manter: usize) {
    let Ok(lista) = std::fs::read_dir(pasta) else { return };
    let mut backups: Vec<PathBuf> = lista
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == EXTENSAO)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(PREFIXO))
        })
        .collect();
    // O nome leva data e hora, então a ordem alfabética é a cronológica.
    backups.sort();
    let sobra = backups.len().saturating_sub(manter);
    for antigo in backups.into_iter().take(sobra) {
        let _ = std::fs::remove_file(antigo);
    }
}

/// Abre o backup com a senha mestra e grava o banco decifrado em `destino` (que não pode
/// existir) e o cabeçalho do cofre ao lado dele (`.cofre`), conferindo a integridade do SQLite.
/// Serve para restaurar e para testar a restauração. Não mexe no banco em uso: quem restaura
/// troca os dois arquivos depois, com o aplicativo fechado.
pub async fn restaurar_backup(arquivo: &Path, senha: &str, destino: &Path) -> Resultado<()> {
    if destino.exists() {
        return Err(ErroBanco::Regra(
            "o destino já existe; escolha um arquivo novo para não sobrescrever dados".into(),
        ));
    }
    let (arquivo, senha, alvo) = (arquivo.to_owned(), senha.to_owned(), destino.to_owned());
    let guarda = ApagarAoSair(destino.to_owned());
    let cofre = tokio::task::spawn_blocking(move || decifrar_arquivo(&arquivo, &senha, &alvo))
        .await
        .map_err(|e| ErroBanco::Regra(format!("falha ao restaurar: {e}")))??;
    conferir_integridade(destino.to_owned(), Some(cofre.chave_banco_hex().to_string())).await?;
    // O banco restaurado só abre junto do cabeçalho do cofre.
    gravar_cabecalho(destino, cofre.cabecalho())?;
    guarda.manter();
    Ok(())
}

fn ler_u32(f: &mut std::fs::File) -> Result<Option<usize>, std::io::Error> {
    let mut b = [0u8; 4];
    match f.read(&mut b[..1])? {
        0 => return Ok(None),
        _ => f.read_exact(&mut b[1..])?,
    }
    Ok(Some(u32::from_be_bytes(b) as usize))
}

fn decifrar_arquivo(arquivo: &Path, senha: &str, alvo: &Path) -> Resultado<Cofre> {
    let invalido = || ErroBanco::Regra("arquivo de backup inválido ou corrompido".into());
    let mut f = std::fs::File::open(arquivo).map_err(erro_io)?;
    let mut magico = [0u8; MAGICO.len()];
    f.read_exact(&mut magico).map_err(|_| invalido())?;
    if magico != MAGICO {
        return Err(invalido());
    }
    let tam_cab = ler_u32(&mut f).map_err(|_| invalido())?.ok_or_else(invalido)?;
    if tam_cab > TETO_CABECALHO {
        return Err(invalido());
    }
    let mut cab = vec![0u8; tam_cab];
    f.read_exact(&mut cab).map_err(|_| invalido())?;
    let cab: CabecalhoCofre = serde_json::from_slice(&cab).map_err(|_| invalido())?;
    let cofre = Cofre::abrir(senha, &cab)?;

    let mut saida = std::fs::File::create(alvo).map_err(erro_io)?;
    let mut indice = 0u64;
    // Lê um bloco à frente para saber se o atual é o último (o AAD depende disso).
    let mut atual = ler_bloco(&mut f).map_err(|_| invalido())?.ok_or_else(invalido)?;
    loop {
        let proximo = ler_bloco(&mut f).map_err(|_| invalido())?;
        let ultimo = proximo.is_none();
        let claro = cofre.decifrar(&atual, &contexto(indice, ultimo))?;
        saida.write_all(&claro).map_err(erro_io)?;
        match proximo {
            Some(p) => {
                atual = p;
                indice += 1;
            }
            None => break,
        }
    }
    saida.sync_all().map_err(erro_io)?;
    Ok(cofre)
}

fn ler_bloco(f: &mut std::fs::File) -> Result<Option<Vec<u8>>, std::io::Error> {
    let Some(tam) = ler_u32(f)? else { return Ok(None) };
    if tam > TETO_BLOCO_CIFRADO {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bloco = vec![0u8; tam];
    f.read_exact(&mut bloco)?;
    Ok(Some(bloco))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ErroCofre;

    fn pasta_temp(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nfse-bkp-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn banco_em_disco(dir: &Path) -> (Banco, Cofre) {
        let (banco, cofre) = Banco::criar_protegido_rapido(dir.join("emissor.db"), "senha-mestra")
            .await
            .unwrap();
        banco.definir_configuracao("marca", "valor-do-teste").await.unwrap();
        (banco, cofre)
    }

    #[tokio::test]
    async fn faz_e_restaura_o_backup() {
        let dir = pasta_temp("ida-e-volta");
        let (banco, cofre) = banco_em_disco(&dir).await;
        let arquivo = banco.fazer_backup(&cofre, &dir.join("copias")).await.unwrap();

        let bytes = std::fs::read(&arquivo).unwrap();
        assert!(!bytes.windows(14).any(|w| w == b"valor-do-teste"), "backup em claro");
        assert!(!bytes.starts_with(b"SQLite format 3"));

        let restaurado = dir.join("restaurado.db");
        restaurar_backup(&arquivo, "senha-mestra", &restaurado).await.unwrap();
        // Restaurar = trocar emissor.db e emissor.cofre pelos gerados; aqui o par vira restaurado.db/.cofre.
        assert!(dir.join("restaurado.cofre").exists());
        assert!(!std::fs::read(&restaurado).unwrap().starts_with(b"SQLite format 3"));
        let (outro, _) = Banco::abrir_protegido(restaurado.clone(), "senha-mestra".into())
            .await
            .unwrap();
        assert_eq!(
            outro.configuracao("marca").await.unwrap().as_deref(),
            Some("valor-do-teste")
        );
        assert_eq!(banco.auditoria(5).await.unwrap()[0].1, "backup_criado");
        banco.fechar().await;
        outro.fechar().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn recusa_senha_errada_adulteracao_e_truncamento() {
        let dir = pasta_temp("recusas");
        let (banco, cofre) = banco_em_disco(&dir).await;
        let arquivo = banco.fazer_backup(&cofre, &dir.join("copias")).await.unwrap();

        let r = restaurar_backup(&arquivo, "outra-senha!", &dir.join("a.db")).await;
        assert!(matches!(r, Err(ErroBanco::Cofre(ErroCofre::SenhaIncorreta))));
        assert!(!dir.join("a.db").exists());

        let mut bytes = std::fs::read(&arquivo).unwrap();
        let truncado = dir.join("truncado.nfbk");
        std::fs::write(&truncado, &bytes[..bytes.len() - 100]).unwrap();
        assert!(
            restaurar_backup(&truncado, "senha-mestra", &dir.join("b.db"))
                .await
                .is_err()
        );
        assert!(!dir.join("b.db").exists());

        *bytes.last_mut().unwrap() ^= 1;
        let adulterado = dir.join("adulterado.nfbk");
        std::fs::write(&adulterado, &bytes).unwrap();
        assert!(
            restaurar_backup(&adulterado, "senha-mestra", &dir.join("c.db"))
                .await
                .is_err()
        );
        assert!(!dir.join("c.db").exists());

        std::fs::write(dir.join("d.db"), b"x").unwrap();
        assert!(
            restaurar_backup(&arquivo, "senha-mestra", &dir.join("d.db"))
                .await
                .is_err()
        );
        banco.fechar().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn poda_so_os_backups_antigos() {
        let dir = pasta_temp("poda");
        for dia in 1..=5 {
            std::fs::write(dir.join(format!("emissor-2026010{dia}-000000.nfbk")), b"x").unwrap();
        }
        std::fs::write(dir.join("outro.txt"), b"x").unwrap();
        podar(&dir, 2);
        let mut restantes: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        restantes.sort();
        assert_eq!(
            restantes,
            [
                "emissor-20260104-000000.nfbk",
                "emissor-20260105-000000.nfbk",
                "outro.txt"
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
