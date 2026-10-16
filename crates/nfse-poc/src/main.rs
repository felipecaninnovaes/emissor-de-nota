//! Prova de conceito da Fase 0: certificado A1, assinatura XMLDSIG e mTLS com a Sefin Nacional.
//!
//! A senha do .pfx vem da variável `NFSE_PFX_SENHA` ou é pedida no terminal (nunca por argumento,
//! para não ficar no histórico do shell).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};

use nfse_cert::CertificadoA1;
use nfse_nacional::{Ambiente, ClienteSefin, ErroSefin};

#[derive(Parser)]
#[command(version, about = "Prova de conceito do emissor de NFS-e Nacional")]
struct Cli {
    #[command(subcommand)]
    comando: Comando,
}

#[derive(Clone, Copy, ValueEnum)]
enum AmbienteArg {
    Restrita,
    Producao,
}

impl From<AmbienteArg> for Ambiente {
    fn from(a: AmbienteArg) -> Self {
        match a {
            AmbienteArg::Restrita => Ambiente::ProducaoRestrita,
            AmbienteArg::Producao => Ambiente::Producao,
        }
    }
}

#[derive(Subcommand)]
enum Comando {
    /// Mostra titular, CNPJ, validade, AC e EKU do certificado.
    CertInfo {
        #[arg(long)]
        pfx: PathBuf,
    },
    /// Assina uma DPS (ou evento) e grava o XML assinado.
    Assinar {
        #[arg(long)]
        pfx: PathBuf,
        #[arg(long)]
        xml: PathBuf,
        /// Id do elemento a assinar; se omitido, usa o primeiro elemento com atributo Id.
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        saida: PathBuf,
    },
    /// Verifica as assinaturas de um XML (DPS assinada ou NFS-e devolvida pela Sefin).
    Verificar {
        #[arg(long)]
        xml: PathBuf,
    },
    /// Testa o mTLS consultando uma DPS pelo Id (404 = conexão ok, DPS inexistente).
    ConsultarDps {
        #[arg(long)]
        pfx: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value = "restrita")]
        ambiente: AmbienteArg,
    },
    /// Envia uma DPS já assinada. Em Produção exige --confirmo-producao.
    Emitir {
        #[arg(long)]
        pfx: PathBuf,
        #[arg(long)]
        xml: PathBuf,
        #[arg(long, value_enum, default_value = "restrita")]
        ambiente: AmbienteArg,
        #[arg(long)]
        confirmo_producao: bool,
        /// Onde gravar o XML da NFS-e autorizada.
        #[arg(long)]
        saida: PathBuf,
    },
}

fn abrir_certificado(caminho: &Path) -> Result<CertificadoA1> {
    let pfx = std::fs::read(caminho).with_context(|| format!("lendo {}", caminho.display()))?;
    let senha = match std::env::var("NFSE_PFX_SENHA") {
        Ok(s) => zeroize::Zeroizing::new(s),
        Err(_) => zeroize::Zeroizing::new(rpassword::prompt_password("Senha do certificado: ")?),
    };
    Ok(CertificadoA1::carregar(&pfx, &senha)?)
}

fn id_do_primeiro_elemento(xml: &str) -> Result<String> {
    let doc = roxmltree::Document::parse(xml)?;
    doc.descendants()
        .find_map(|n| n.attribute("Id"))
        .map(str::to_owned)
        .context("nenhum elemento com atributo Id no XML")
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().comando {
        Comando::CertInfo { pfx } => {
            let cert = abrir_certificado(&pfx)?;
            let i = cert.info();
            let agora = Utc::now();
            println!("Titular:       {}", i.titular);
            println!(
                "CNPJ:          {}",
                i.cnpj
                    .as_ref()
                    .map_or("(não encontrado no certificado)".into(), |c| c.to_string())
            );
            println!("Responsável:   {}", i.responsavel.as_deref().unwrap_or("-"));
            println!("AC emissora:   {}", i.emissor);
            println!("Número série:  {}", i.numero_serie);
            println!("Thumbprint:    {}", i.thumbprint);
            println!("Válido de:     {}", i.valido_de.format("%d/%m/%Y %H:%M"));
            println!(
                "Válido até:    {} ({} dias)",
                i.valido_ate.format("%d/%m/%Y %H:%M"),
                i.dias_para_vencer(agora)
            );
            println!(
                "Client Auth:   {}",
                if i.client_auth {
                    "sim"
                } else {
                    "NÃO — a Sefin vai recusar o mTLS"
                }
            );
            if i.vencido_em(agora) {
                println!("ATENÇÃO: certificado fora da validade.");
            }
        }
        Comando::Assinar { pfx, xml, id, saida } => {
            let conteudo = std::fs::read_to_string(&xml).with_context(|| format!("lendo {}", xml.display()))?;
            let id = match id {
                Some(id) => id,
                None => id_do_primeiro_elemento(&conteudo)?,
            };
            let cert = abrir_certificado(&pfx)?;
            let assinado = nfse_xml::assinar(&conteudo, &id, &cert)?;
            nfse_xml::verificar(&assinado).context("a assinatura gerada não passou na verificação")?;
            std::fs::write(&saida, assinado)?;
            println!("Assinado ({id}) em {}", saida.display());
        }
        Comando::Verificar { xml } => {
            let conteudo = std::fs::read_to_string(&xml)?;
            for der in nfse_xml::verificar(&conteudo)? {
                let info = nfse_cert::InfoCertificado::de_der(&der)?;
                println!("Assinatura válida — {} (AC: {})", info.titular, info.emissor);
            }
        }
        Comando::ConsultarDps { pfx, id, ambiente } => {
            let cert = abrir_certificado(&pfx)?;
            let cliente = ClienteSefin::novo(&cert, ambiente.into())?;
            match cliente.consultar_dps(&id).await? {
                Some(chave) => println!("DPS com NFS-e gerada. Chave de acesso: {chave}"),
                None => println!("Conexão mTLS ok; não há NFS-e para esta DPS."),
            }
        }
        Comando::Emitir {
            pfx,
            xml,
            ambiente,
            confirmo_producao,
            saida,
        } => {
            if matches!(ambiente, AmbienteArg::Producao) && !confirmo_producao {
                bail!("emissão em Produção gera nota com valor fiscal; repita com --confirmo-producao");
            }
            let conteudo = std::fs::read_to_string(&xml)?;
            nfse_xml::verificar(&conteudo).context("a DPS precisa estar assinada")?;
            let id = id_do_primeiro_elemento(&conteudo)?;
            let cert = abrir_certificado(&pfx)?;
            let cliente = ClienteSefin::novo(&cert, ambiente.into())?;
            match cliente.emitir(&conteudo).await {
                Ok(nfse) => {
                    std::fs::write(&saida, &nfse.xml)?;
                    println!("Autorizada. Chave: {}  XML em {}", nfse.chave_acesso, saida.display());
                    for a in nfse.alertas {
                        println!(
                            "Alerta {}: {}",
                            a.codigo.unwrap_or_default(),
                            a.descricao.unwrap_or_default()
                        );
                    }
                }
                Err(ErroSefin::Incerto(motivo)) => {
                    eprintln!("Sem resposta da Sefin ({motivo}). Reconciliando pelo Id da DPS...");
                    match cliente.consultar_dps(&id).await? {
                        Some(chave) => {
                            println!("A nota FOI autorizada. Chave: {chave} (baixe com a consulta da NFS-e)")
                        }
                        None => println!("A Sefin não tem NFS-e para {id}; é seguro reenviar."),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(())
}
