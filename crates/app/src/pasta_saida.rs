//! Troca da pasta das notas (XML, PDF e CSVs). O banco guarda só a raiz (`pasta_saida`) e o
//! caminho de cada nota é montado a partir dela, então trocar a pasta sem levar os arquivos faz
//! as notas já emitidas deixarem de abrir pelo aplicativo.
//!
//! A migração é em duas etapas, para uma falha no meio não deixar nota sem cópia: `copiar` leva
//! tudo para a pasta nova sem apagar nada; só depois de a configuração ser gravada, `apagar_origens`
//! tira da pasta antiga o que foi copiado.

use std::io;
use std::path::{Path, PathBuf};

/// Resultado da cópia: o que já está na pasta nova (e pode sair da antiga) e o que não foi levado
/// porque a pasta nova já tem um arquivo diferente com o mesmo nome.
#[derive(Debug, Default)]
pub struct Copia {
    pub copiados: Vec<PathBuf>,
    pub conflitos: Vec<PathBuf>,
}

/// Quantos arquivos há na pasta, contando as subpastas. Pasta inexistente conta zero.
pub fn contar_arquivos(pasta: &Path) -> io::Result<u64> {
    if !pasta.is_dir() {
        return Ok(0);
    }
    let mut total = 0;
    for entrada in std::fs::read_dir(pasta)? {
        let entrada = entrada?;
        if entrada.file_type()?.is_dir() {
            total += contar_arquivos(&entrada.path())?;
        } else {
            total += 1;
        }
    }
    Ok(total)
}

/// Mesma pasta, ou uma dentro da outra: nesses casos não há o que (ou não dá para) migrar.
/// `nova` precisa existir, para os dois caminhos serem comparados já resolvidos.
pub fn relacao(antiga: &Path, nova: &Path) -> io::Result<Relacao> {
    if !antiga.is_dir() {
        return Ok(Relacao::Separadas);
    }
    let antiga = std::fs::canonicalize(antiga)?;
    let nova = std::fs::canonicalize(nova)?;
    Ok(if antiga == nova {
        Relacao::Mesma
    } else if nova.starts_with(&antiga) || antiga.starts_with(&nova) {
        Relacao::Aninhadas
    } else {
        Relacao::Separadas
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Relacao {
    Mesma,
    Aninhadas,
    Separadas,
}

/// Copia o conteúdo de `antiga` para `nova`, mantendo as subpastas, sem sobrescrever nada e sem
/// apagar nada. Arquivo que já existe no destino com o mesmo conteúdo conta como copiado.
pub fn copiar(antiga: &Path, nova: &Path) -> io::Result<Copia> {
    let mut copia = Copia::default();
    copiar_em(antiga, nova, &mut copia)?;
    Ok(copia)
}

fn copiar_em(origem: &Path, destino: &Path, copia: &mut Copia) -> io::Result<()> {
    std::fs::create_dir_all(destino)?;
    for entrada in std::fs::read_dir(origem)? {
        let entrada = entrada?;
        let de = entrada.path();
        let para = destino.join(entrada.file_name());
        if entrada.file_type()?.is_dir() {
            copiar_em(&de, &para, copia)?;
        } else if para.exists() {
            if std::fs::read(&de)? == std::fs::read(&para)? {
                copia.copiados.push(de);
            } else {
                copia.conflitos.push(de);
            }
        } else {
            std::fs::copy(&de, &para)?;
            // XML e PDF fiscais: a cópia vai ao disco antes de a original poder ser apagada.
            std::fs::File::open(&para)?.sync_all()?;
            copia.copiados.push(de);
        }
    }
    Ok(())
}

/// Apaga da pasta antiga os arquivos já copiados e as subpastas que ficaram vazias (a própria
/// pasta antiga inclusive). Conflitos e arquivos novos ficam onde estão.
pub fn apagar_origens(antiga: &Path, copia: &Copia) -> io::Result<()> {
    for arquivo in &copia.copiados {
        std::fs::remove_file(arquivo)?;
    }
    remover_pastas_vazias(antiga);
    Ok(())
}

fn remover_pastas_vazias(pasta: &Path) {
    if let Ok(entradas) = std::fs::read_dir(pasta) {
        for entrada in entradas.flatten() {
            if entrada.file_type().is_ok_and(|t| t.is_dir()) {
                remover_pastas_vazias(&entrada.path());
            }
        }
    }
    // Só sai se estiver vazia; o que restou fica para o operador.
    let _ = std::fs::remove_dir(pasta);
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn pasta_temporaria() -> PathBuf {
        static CONTADOR: AtomicU32 = AtomicU32::new(0);
        let pasta = std::env::temp_dir().join(format!(
            "emissor-pasta-saida-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    fn gravar(caminho: &Path, conteudo: &str) {
        std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        std::fs::write(caminho, conteudo).unwrap();
    }

    #[test]
    fn move_as_notas_com_as_subpastas() {
        let base = pasta_temporaria();
        let antiga = base.join("antiga");
        let nova = base.join("nova");
        gravar(&antiga.join("EMPRESA - 1/2026-11/NFSe-1.xml"), "xml");
        gravar(&antiga.join("EMPRESA - 1/2026-11/NFSe-1.pdf"), "pdf");
        gravar(&antiga.join("modelo.csv"), "csv");
        assert_eq!(contar_arquivos(&antiga).unwrap(), 3);

        let copia = copiar(&antiga, &nova).unwrap();
        assert_eq!(copia.copiados.len(), 3);
        assert!(copia.conflitos.is_empty());
        // Antes de apagar, as duas pastas têm tudo.
        assert_eq!(contar_arquivos(&antiga).unwrap(), 3);

        apagar_origens(&antiga, &copia).unwrap();
        assert!(!antiga.exists());
        assert_eq!(
            std::fs::read_to_string(nova.join("EMPRESA - 1/2026-11/NFSe-1.xml")).unwrap(),
            "xml"
        );
        assert_eq!(contar_arquivos(&nova).unwrap(), 3);
    }

    #[test]
    fn nao_sobrescreve_arquivo_diferente_na_pasta_nova() {
        let base = pasta_temporaria();
        let antiga = base.join("antiga");
        let nova = base.join("nova");
        gravar(&antiga.join("a.xml"), "da antiga");
        gravar(&nova.join("a.xml"), "da nova");
        gravar(&antiga.join("b.xml"), "igual");
        gravar(&nova.join("b.xml"), "igual");

        let copia = copiar(&antiga, &nova).unwrap();
        assert_eq!(copia.conflitos, vec![antiga.join("a.xml")]);
        assert_eq!(copia.copiados, vec![antiga.join("b.xml")]);
        apagar_origens(&antiga, &copia).unwrap();
        assert_eq!(std::fs::read_to_string(nova.join("a.xml")).unwrap(), "da nova");
        assert_eq!(std::fs::read_to_string(antiga.join("a.xml")).unwrap(), "da antiga");
        assert!(!antiga.join("b.xml").exists());
    }

    #[test]
    fn reconhece_mesma_pasta_e_pastas_aninhadas() {
        let base = pasta_temporaria();
        let antiga = base.join("antiga");
        std::fs::create_dir_all(antiga.join("dentro")).unwrap();
        std::fs::create_dir_all(base.join("outra")).unwrap();
        assert_eq!(relacao(&antiga, &antiga).unwrap(), Relacao::Mesma);
        assert_eq!(relacao(&antiga, &antiga.join("dentro")).unwrap(), Relacao::Aninhadas);
        assert_eq!(relacao(&antiga, &base).unwrap(), Relacao::Aninhadas);
        assert_eq!(relacao(&antiga, &base.join("outra")).unwrap(), Relacao::Separadas);
        assert_eq!(relacao(&base.join("nao-existe"), &antiga).unwrap(), Relacao::Separadas);
    }
}
