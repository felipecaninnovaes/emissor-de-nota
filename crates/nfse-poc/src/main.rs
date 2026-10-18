//! Prova de conceito da Fase 0: certificado A1, assinatura XMLDSIG e mTLS com a Sefin Nacional.
//!
//! A senha do .pfx vem da variável `NFSE_PFX_SENHA` ou é pedida no terminal (nunca por argumento,
//! para não ficar no histórico do shell).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Timelike, Utc};
use clap::{Parser, Subcommand, ValueEnum};

use nfse_cert::CertificadoA1;
use nfse_core::DpsId;
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

#[derive(Clone, Copy, ValueEnum)]
enum MotivoArg {
    ErroNaEmissao,
    ServicoNaoPrestado,
    Outros,
}

impl From<MotivoArg> for nfse_xml::MotivoCancelamento {
    fn from(m: MotivoArg) -> Self {
        match m {
            MotivoArg::ErroNaEmissao => Self::ErroNaEmissao,
            MotivoArg::ServicoNaoPrestado => Self::ServicoNaoPrestado,
            MotivoArg::Outros => Self::Outros,
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
    /// Gera o XML da DPS a partir de um JSON (ver exemplos/dps-exemplo.json) e, com --pfx, assina.
    GerarDps {
        #[arg(long)]
        entrada: PathBuf,
        #[arg(long)]
        saida: PathBuf,
        /// Usa a data/hora atual (Brasília, 1 minuto atrás) em vez de `emissao` do JSON.
        #[arg(long)]
        agora: bool,
        /// Certificado para assinar a DPS gerada.
        #[arg(long)]
        pfx: Option<PathBuf>,
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
    /// Gera o PDF do DANFSe (NT 008/2026) a partir do XML da NFS-e.
    Danfse {
        #[arg(long)]
        xml: PathBuf,
        #[arg(long)]
        saida: PathBuf,
        /// Imprime a marca d'água CANCELADA.
        #[arg(long)]
        cancelada: bool,
        /// Logomarca oficial da NFS-e (PNG).
        #[arg(long)]
        logo: Option<PathBuf>,
    },
    /// Verifica as assinaturas de um XML (DPS assinada ou NFS-e devolvida pela Sefin).
    Verificar {
        #[arg(long)]
        xml: PathBuf,
    },
    /// Cancela uma NFS-e pela chave (evento e101101). Com --somente-gerar, só grava o pedido assinado.
    Cancelar {
        #[arg(long)]
        pfx: PathBuf,
        #[arg(long)]
        chave: String,
        /// erro-na-emissao, servico-nao-prestado ou outros.
        #[arg(long, value_enum)]
        motivo: MotivoArg,
        /// Justificativa (15 a 255 caracteres).
        #[arg(long)]
        justificativa: String,
        #[arg(long, value_enum, default_value = "restrita")]
        ambiente: AmbienteArg,
        #[arg(long)]
        confirmo_producao: bool,
        /// Grava o pedido assinado neste arquivo e não envia.
        #[arg(long)]
        somente_gerar: Option<PathBuf>,
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
    /// Consulta os eventos de uma NFS-e (só leitura) e mostra as respostas brutas da Sefin:
    /// diagnóstico do formato da consulta de eventos.
    ConsultarEventos {
        #[arg(long)]
        pfx: PathBuf,
        #[arg(long)]
        chave: String,
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
        Comando::GerarDps {
            entrada,
            saida,
            agora,
            pfx,
        } => {
            let json = std::fs::read_to_string(&entrada).with_context(|| format!("lendo {}", entrada.display()))?;
            let mut dps: nfse_core::Dps = serde_json::from_str(&json).context("JSON da DPS inválido")?;
            if agora {
                // Um minuto de folga: dhEmi posterior ao relógio da Sefin é rejeitado.
                let brasilia = chrono::FixedOffset::west_opt(3 * 3600).expect("fuso fixo");
                let instante = Utc::now() - chrono::Duration::minutes(1);
                dps.emissao = instante
                    .with_timezone(&brasilia)
                    .with_nanosecond(0)
                    .expect("nanossegundo zero");
            }
            let (mut xml, id) = nfse_xml::gerar_dps(&dps)?;
            if let Some(pfx) = pfx {
                let cert = abrir_certificado(&pfx)?;
                if let nfse_core::modelo::Documento::Cnpj(cnpj) = &dps.prestador.documento
                    && !cert.info().pertence_a(cnpj)
                {
                    bail!("o certificado ({}) não é do prestador {cnpj}", cert.info().titular);
                }
                xml = nfse_xml::assinar(&xml, id.as_str(), &cert)?;
            }
            std::fs::write(&saida, xml)?;
            println!("DPS {id} gravada em {}", saida.display());
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
        Comando::Danfse {
            xml,
            saida,
            cancelada,
            logo,
        } => {
            let conteudo = std::fs::read_to_string(&xml)?;
            let opcoes = nfse_pdf::Opcoes {
                situacao: if cancelada {
                    nfse_pdf::Situacao::Cancelada
                } else {
                    nfse_pdf::Situacao::Normal
                },
                logo_png: logo.map(std::fs::read).transpose()?,
            };
            let pdf = nfse_pdf::gerar_danfse(&conteudo, &opcoes, &nfse_pdf::Fontes::do_sistema()?)?;
            std::fs::write(&saida, pdf)?;
            println!("DANFSe gravado em {}", saida.display());
        }
        Comando::Verificar { xml } => {
            let conteudo = std::fs::read_to_string(&xml)?;
            for der in nfse_xml::verificar(&conteudo)? {
                let info = nfse_cert::InfoCertificado::de_der(&der)?;
                println!("Assinatura válida — {} (AC: {})", info.titular, info.emissor);
            }
        }
        Comando::Cancelar {
            pfx,
            chave,
            motivo,
            justificativa,
            ambiente,
            confirmo_producao,
            somente_gerar,
        } => {
            if somente_gerar.is_none() && matches!(ambiente, AmbienteArg::Producao) && !confirmo_producao {
                bail!("cancelar em Produção afeta uma nota com valor fiscal; repita com --confirmo-producao");
            }
            let cert = abrir_certificado(&pfx)?;
            let cnpj = cert.info().cnpj.clone().context("o certificado não tem CNPJ")?;
            let pedido = nfse_xml::PedidoCancelamento {
                ambiente: match ambiente {
                    AmbienteArg::Restrita => nfse_core::modelo::Ambiente::Homologacao,
                    AmbienteArg::Producao => nfse_core::modelo::Ambiente::Producao,
                },
                versao_aplicativo: concat!("emissor-nfse/", env!("CARGO_PKG_VERSION")).into(),
                quando: nfse_emissor::agora_brasilia(),
                autor: nfse_core::modelo::Documento::Cnpj(cnpj),
                chave: chave.clone(),
                motivo: motivo.into(),
                descricao: justificativa,
            };
            let (xml, id) = nfse_xml::gerar_pedido_cancelamento(&pedido)?;
            let assinado = nfse_xml::assinar(&xml, &id, &cert)?;
            if let Some(arquivo) = somente_gerar {
                std::fs::write(&arquivo, assinado)?;
                println!("Pedido {id} gravado em {} (não enviado)", arquivo.display());
                return Ok(());
            }
            let cliente = ClienteSefin::novo(&cert, ambiente.into())?;
            let evento = cliente.registrar_evento(&chave, &assinado).await?;
            println!("Cancelamento registrado. Evento:\n{evento}");
        }
        Comando::ConsultarDps { pfx, id, ambiente } => {
            let id = DpsId::parse(&id).context("Id da DPS inválido")?;
            let id = id.as_str();
            let cert = abrir_certificado(&pfx)?;
            let cliente = ClienteSefin::novo(&cert, ambiente.into())?;
            match cliente.consultar_dps(id).await? {
                Some(chave) => println!("DPS com NFS-e gerada. Chave de acesso: {chave}"),
                None => println!("Conexão mTLS ok; não há NFS-e para esta DPS."),
            }
        }
        Comando::ConsultarEventos { pfx, chave, ambiente } => {
            let cert = abrir_certificado(&pfx)?;
            let cliente = ClienteSefin::novo(&cert, ambiente.into())?;
            for caminho in [
                format!("/nfse/{chave}/eventos"),
                format!("/nfse/{chave}/eventos/101101"),
                format!("/nfse/{chave}/eventos/101101/1"),
            ] {
                let (status, corpo) = cliente.consulta_bruta(&caminho).await?;
                println!("GET {caminho} -> HTTP {status}\n{corpo}\n");
            }
            match cliente.consultar_evento(&chave, "101101", 1).await? {
                Some(xml) => println!("Cancelamento encontrado. Evento:\n{xml}"),
                None => println!("A Sefin não tem evento de cancelamento para esta NFS-e."),
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
            DpsId::parse(&id).context("Id da DPS inválido no XML")?;
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
