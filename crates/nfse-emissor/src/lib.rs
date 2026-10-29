//! Emissão confiável: junta banco, geração/assinatura do XML e Sefin.
//!
//! Garantias:
//! - nenhum número é gasto com DPS que não passa na validação local;
//! - a DPS é gravada como `enviando` antes da chamada, então uma queda no meio é retomada
//!   por [`Emissor::reconciliar_pendentes`];
//! - em resposta incerta, nada é reenviado antes de consultar a Sefin pelo Id da DPS;
//! - a NFS-e é gravada exatamente como a Sefin devolveu.

pub mod arquivo;
pub mod email;
pub mod importacao;
pub mod planilha;
pub mod planilha_servicos;
pub mod planilha_tomadores;

mod cancelamento;
mod copia;
mod diagnostico;
mod envio_email;
mod montagem;
mod reconciliacao;
mod substituicao;

pub use copia::Refazer;
pub use diagnostico::mascarar_xml;
pub use montagem::{
    RetencoesDispensaveis, VARIAVEIS_DESCRICAO, agora_brasilia, aplicar_mes_ref, federais_com_escolha,
    federais_da_nota, montar_dps, retencoes_dispensaveis, texto_variaveis_desconhecidas, tributacao_da_nota,
    variaveis_desconhecidas,
};
pub use reconciliacao::JANELA_RECONCILIACAO;
pub use substituicao::substituicao_do_pedido;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};

use nfse_cert::CertificadoA1;
use nfse_core::Valor;
use nfse_core::modelo::{Ambiente, Obra};
use nfse_db::dps::{NfseGravada, NfseRecebida};
use nfse_db::empresas::Empresa;
use nfse_db::tomadores::TomadorVinculado;
use nfse_db::{Banco, Cofre, ErroBanco};
use nfse_nacional::{ClienteSefin, ErroSefin, MensagemSefin, Sefin};

pub const VERSAO_APLICATIVO: &str = concat!("emissor-nfse/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum ErroEmissao {
    #[error(transparent)]
    Banco(#[from] ErroBanco),
    #[error(transparent)]
    DpsInvalida(#[from] nfse_xml::DpsInvalida),
    #[error(transparent)]
    Assinatura(#[from] nfse_xml::ErroAssinatura),
    #[error(transparent)]
    LeituraNfse(#[from] nfse_xml::ErroLeituraNfse),
    #[error(transparent)]
    PedidoEvento(#[from] nfse_xml::PedidoInvalido),
    #[error("{0}")]
    Regra(String),
    #[error("rejeitada pela Sefin: {mensagem}")]
    Rejeitada { dps_id: Option<i64>, mensagem: String },
    #[error("não foi possível falar com a Sefin ({motivo}); nada foi emitido, pode tentar de novo")]
    SemConexao { motivo: String },
    #[error("Sefin: {0}")]
    Sefin(ErroSefin),
}

#[derive(Debug)]
pub enum ResultadoEmissao {
    /// `avisos`: problemas que não desfazem a emissão (assinatura da Sefin não conferiu, falha ao
    /// gravar o arquivo na pasta). Também ficam na auditoria.
    Autorizada { nfse: NfseGravada, avisos: Vec<String> },
    /// Sem resposta e a consulta também falhou: a DPS fica pendente para reconciliar depois.
    Incerta { dps_id: i64, motivo: String },
}

/// Cria o cliente da Sefin para um certificado e ambiente (o real usa mTLS; os testes, um simulado).
pub trait Conector {
    type S: Sefin;
    fn conectar(&self, certificado: &CertificadoA1, ambiente: Ambiente) -> Result<Self::S, ErroSefin>;
}

pub struct ConectorSefin;

impl Conector for ConectorSefin {
    type S = ClienteSefin;

    fn conectar(&self, certificado: &CertificadoA1, ambiente: Ambiente) -> Result<ClienteSefin, ErroSefin> {
        let ambiente = match ambiente {
            Ambiente::Producao => nfse_nacional::Ambiente::Producao,
            Ambiente::Homologacao => nfse_nacional::Ambiente::ProducaoRestrita,
        };
        ClienteSefin::novo(certificado, ambiente)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoEmissao {
    pub empresa_id: i64,
    /// `pessoa_id` do tomador vinculado à empresa.
    pub tomador: Option<i64>,
    pub servico_id: i64,
    pub valor: Valor,
    /// Substitui a descrição padrão do serviço.
    pub descricao: Option<String>,
    /// Mês usado em `{mes_ref}`; padrão: mês anterior ao da emissão.
    pub mes_referencia: Option<NaiveDate>,
    /// Grupo de obra (construção civil, E0370); vai só nesta nota.
    pub obra: Option<Obra>,
    /// Município (IBGE) onde o serviço foi prestado (`cLocPrestacao`); `None` = o da empresa.
    pub local_prestacao: Option<String>,
    pub informacoes_complementares: Option<String>,
    /// Retenções desta nota; o que não for dito vem do cadastro do tomador.
    pub reter: ReterNota,
    pub origem: Origem,
    /// Nota autorizada que esta substitui (grupo `subst`).
    pub substituir: Option<SubstituirNota>,
}

/// Substituição pedida na emissão: a NFS-e gravada e o motivo (`cMotivo` 01 a 05 ou 99).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstituirNota {
    pub nfse_id: i64,
    pub motivo: String,
    /// `xMotivo`, de 15 a 255 caracteres.
    pub descricao_motivo: Option<String>,
}

/// Escolha de retenção feita na emissão. `None` = o que o cadastro do tomador diz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReterNota {
    pub federais: Option<bool>,
    pub iss: Option<bool>,
    /// Reter o IRRF mesmo sem passar de R$ 10,00 (outras notas do período, somadas, passam).
    pub irrf_ate_o_limite: bool,
    /// Reter PIS, COFINS e CSLL mesmo com a soma sem passar de R$ 10,00.
    pub contribuicoes_ate_o_limite: bool,
}

impl ReterNota {
    /// `(retém federais, retém ISS)` desta nota.
    pub fn efetiva(self, tomador: Option<&TomadorVinculado>) -> (bool, bool) {
        (
            self.federais
                .unwrap_or_else(|| tomador.is_some_and(|t| t.retem_federais)),
            self.iss.unwrap_or_else(|| tomador.is_some_and(|t| t.retem_iss)),
        )
    }
}

/// De onde vem o pedido. Recorrência e planilha ficam ligadas à DPS desde a reserva do número
/// (antes do envio), para que o mesmo item nunca gere duas notas.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Origem {
    #[default]
    Manual,
    /// Nota recorrente: uma por mês de referência (`mes_referencia` obrigatório).
    Recorrencia(i64),
    /// Linha de planilha; `chave` identifica o conteúdo da linha.
    Planilha {
        chave: String,
        arquivo: Option<String>,
        linha: Option<i64>,
    },
}

fn mensagem_rejeicao(erros: &[MensagemSefin]) -> String {
    erros
        .iter()
        .map(|e| {
            let codigo = e.codigo.as_deref().unwrap_or("?");
            let mut m = format!("{codigo} {}", e.descricao.as_deref().unwrap_or("").trim());
            if let Some(c) = e.complemento.as_deref().filter(|c| !c.trim().is_empty()) {
                m.push_str(&format!(" ({})", c.trim()));
            }
            if let Some(o) = nfse_nacional::mensagens::orientacao(codigo) {
                m.push_str(&format!(" → {o}"));
            }
            m
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub struct Emissor<C: Conector> {
    banco: Banco,
    cofre: Cofre,
    conector: C,
    pasta: Option<PathBuf>,
    /// Fontes do DANFSe (carregadas junto com a pasta; falha vira aviso na hora de gerar o PDF).
    fontes: Option<Result<nfse_pdf::Fontes, String>>,
    logo: Option<Vec<u8>>,
    /// Identifica o aplicativo nos XML (`verAplic`); o app informa a própria versão.
    versao_aplicativo: String,
    /// Ver [`JANELA_RECONCILIACAO`]; os testes zeram.
    janela_reconciliacao: chrono::Duration,
}

impl<C: Conector> Emissor<C> {
    pub fn novo(banco: Banco, cofre: Cofre, conector: C) -> Self {
        Self {
            banco,
            cofre,
            conector,
            pasta: None,
            fontes: None,
            logo: None,
            versao_aplicativo: VERSAO_APLICATIVO.into(),
            janela_reconciliacao: JANELA_RECONCILIACAO,
        }
    }

    /// Versão do aplicativo que vai nos XML, no formato `emissor-nfse/0.2.1`.
    pub fn com_versao(mut self, versao: impl Into<String>) -> Self {
        self.versao_aplicativo = versao.into();
        self
    }

    /// Pasta raiz onde cada nota é gravada em `<empresa>/<AAAA-MM>/`.
    pub fn com_pasta(mut self, raiz: impl Into<PathBuf>) -> Self {
        self.pasta = Some(raiz.into());
        self.fontes = Some(nfse_pdf::Fontes::do_sistema().map_err(|e| e.to_string()));
        self
    }

    /// Troca a pasta de saída sem reabrir o cofre.
    pub fn definir_pasta(&mut self, raiz: impl Into<PathBuf>) {
        self.pasta = Some(raiz.into());
        if self.fontes.is_none() {
            self.fontes = Some(nfse_pdf::Fontes::do_sistema().map_err(|e| e.to_string()));
        }
    }

    pub fn pasta(&self) -> Option<&std::path::Path> {
        self.pasta.as_deref()
    }

    pub fn cofre(&self) -> &Cofre {
        &self.cofre
    }

    /// Caminho do XML da nota na pasta de saída (o PDF fica ao lado, com extensão .pdf).
    pub async fn caminho_xml(&self, nfse_id: i64) -> Result<Option<PathBuf>, ErroEmissao> {
        let Some(raiz) = &self.pasta else { return Ok(None) };
        let nfse = self.banco.nfse(nfse_id).await?;
        let empresa = self
            .banco
            .empresa(self.banco.dps(nfse.dps_id).await?.empresa_id)
            .await?;
        let competencia = nfse_xml::ler_nfse(&nfse.xml)?
            .competencia
            .and_then(|c| NaiveDate::parse_from_str(&c, "%Y-%m-%d").ok())
            .unwrap_or_else(|| agora_brasilia().date_naive());
        Ok(Some(arquivo::pasta_mes(raiz, &empresa, competencia).join(
            arquivo::nome_xml_nfse(nfse.numero.as_deref().unwrap_or("sem-numero"), &nfse.chave),
        )))
    }

    /// Logomarca oficial da NFS-e (PNG) para o cabeçalho do DANFSe.
    pub fn com_logo(mut self, png: Vec<u8>) -> Self {
        self.logo = Some(png);
        self
    }

    /// DANFSe em memória; `None` sem pasta configurada (as fontes são carregadas com ela).
    /// A renderização é CPU-bound: roda numa thread de bloqueio, fora do executor assíncrono.
    async fn gerar_pdf(&self, xml: &str, situacao: nfse_pdf::Situacao) -> Result<Option<Vec<u8>>, String> {
        let fontes = match &self.fontes {
            Some(Ok(f)) => f.clone(),
            Some(Err(e)) => return Err(e.clone()),
            None => return Ok(None),
        };
        let opcoes = nfse_pdf::Opcoes {
            situacao,
            logo_png: self.logo.clone(),
        };
        let xml = xml.to_owned();
        tokio::task::spawn_blocking(move || nfse_pdf::gerar_danfse(&xml, &opcoes, &fontes))
            .await
            .map_err(|e| format!("falha ao gerar o DANFSe: {e}"))?
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// Gera o DANFSe e grava ao lado do XML (mesmo nome, extensão .pdf).
    async fn gravar_pdf(
        &self,
        caminho_xml: &std::path::Path,
        xml: &str,
        situacao: nfse_pdf::Situacao,
    ) -> Result<(), String> {
        let Some(pdf) = self.gerar_pdf(xml, situacao).await? else {
            return Ok(());
        };
        let caminho = caminho_xml.with_extension("pdf");
        tokio::task::spawn_blocking(move || {
            arquivo::gravar(&caminho, &pdf).map_err(|e| format!("{}: {e}", caminho.display()))
        })
        .await
        .map_err(|e| format!("falha ao gravar o DANFSe: {e}"))?
    }

    pub fn banco(&self) -> &Banco {
        &self.banco
    }

    /// O ambiente vem da DPS quando ela já existe: a empresa pode ter mudado de ambiente depois.
    async fn conectar(
        &self,
        empresa: &Empresa,
        ambiente: Ambiente,
    ) -> Result<(std::sync::Arc<CertificadoA1>, C::S), ErroEmissao> {
        let cert = self.banco.certificado_ativo(&self.cofre, empresa.id).await?;
        let sefin = self.conector.conectar(&cert, ambiente).map_err(ErroEmissao::Sefin)?;
        Ok((std::sync::Arc::new(cert), sefin))
    }

    pub async fn emitir(&self, pedido: &PedidoEmissao) -> Result<ResultadoEmissao, ErroEmissao> {
        let empresa = self.banco.empresa(pedido.empresa_id).await?;
        let servico = self.banco.servico(pedido.servico_id).await?;
        if servico.empresa_id != empresa.id {
            return Err(ErroEmissao::Regra("o serviço escolhido é de outra empresa".into()));
        }
        let tomador = match pedido.tomador {
            Some(id) => Some(self.banco.tomador(empresa.id, id).await?),
            None => None,
        };

        // Recorrência e planilha: não emite de novo o que já tem nota autorizada ou em andamento.
        let mes_recorrencia = match &pedido.origem {
            Origem::Recorrencia(_) => Some(
                pedido
                    .mes_referencia
                    .ok_or_else(|| ErroEmissao::Regra("nota recorrente exige o mês de referência".into()))?
                    .format("%Y-%m")
                    .to_string(),
            ),
            _ => None,
        };
        let anterior = match (&pedido.origem, &mes_recorrencia) {
            (Origem::Recorrencia(id), Some(mes)) => self.banco.tentativa_no_mes(*id, mes).await?,
            (Origem::Planilha { chave, .. }, _) => self.banco.tentativa_importada(chave).await?,
            _ => None,
        };
        if let Some(t) = anterior.filter(|t| !t.libera_o_mes()) {
            return Err(ErroEmissao::Regra(format!(
                "já existe nota {} para esta {}",
                t.situacao.texto(),
                match pedido.origem {
                    Origem::Recorrencia(_) => "recorrência neste mês",
                    _ => "linha da planilha",
                }
            )));
        }

        // Valida antes de reservar número: DPS inválida não gasta numeração.
        let mut modelo = montar_dps(&empresa, tomador.as_ref(), &servico, pedido, agora_brasilia());
        if let Some(s) = &pedido.substituir {
            modelo.substituicao = Some(substituicao_do_pedido(&self.banco, &empresa, s).await?);
        }
        let desconhecidas = variaveis_desconhecidas(&modelo.servico.descricao);
        if !desconhecidas.is_empty() {
            return Err(ErroEmissao::Regra(texto_variaveis_desconhecidas(&desconhecidas)));
        }
        modelo.versao_aplicativo = self.versao_aplicativo.clone();
        modelo.leiaute = self.banco.leiaute_dps(empresa.ambiente).await?;
        nfse_xml::gerar_dps(&modelo)?;
        tracing::info!(
            empresa = empresa.id,
            ambiente = ?empresa.ambiente,
            leiaute = modelo.leiaute.codigo(),
            "emissão: DPS validada"
        );
        let (cert, sefin) = self.conectar(&empresa, empresa.ambiente).await?;

        let reserva = self.banco.reservar_dps(empresa.id).await?;
        let vinculo = match (&pedido.origem, &mes_recorrencia) {
            (Origem::Recorrencia(id), Some(mes)) => self.banco.registrar_emissao_recorrente(*id, mes, reserva.id).await,
            (Origem::Planilha { chave, arquivo, linha }, _) => {
                self.banco
                    .registrar_emissao_importada(chave, arquivo.as_deref(), *linha, reserva.id)
                    .await
            }
            _ => Ok(()),
        };
        if let Err(e) = vinculo {
            // Outra emissão do mesmo item começou entre a conferência e a reserva.
            self.banco
                .registrar_rejeicao(reserva.id, &format!("não enviada: {e}"))
                .await?;
            return Err(e.into());
        }
        modelo.serie = reserva.serie.clone();
        modelo.numero = reserva.numero;
        tracing::info!(dps = reserva.id, serie = %reserva.serie, numero = reserva.numero, "emissão: número reservado");
        let preparo = async {
            let (xml, id) = nfse_xml::gerar_dps(&modelo)?;
            let assinado = assinar_em_thread(xml, id.as_str().to_owned(), cert.clone()).await?;
            self.banco.registrar_envio(reserva.id, &modelo, &assinado).await?;
            Ok::<_, ErroEmissao>(assinado)
        };
        let assinado = match preparo.await {
            Ok(assinado) => assinado,
            Err(e) => {
                tracing::error!(dps = reserva.id, erro = %e, "emissão: falha ao preparar a DPS");
                // Não chegou à Sefin: a reserva não pode ficar presa (bloquearia a recorrência ou a linha).
                self.banco
                    .registrar_rejeicao(reserva.id, &format!("não enviada: {e}"))
                    .await?;
                return Err(e);
            }
        };

        tracing::info!(dps = reserva.id, "emissão: enviando à Sefin");
        let resultado = sefin.emitir(&assinado).await;
        self.registrar_resposta(reserva.id, &resultado).await;
        match resultado {
            Ok(nfse) => match self.gravar(reserva.id, &nfse.chave_acesso, &nfse.xml).await {
                Ok((nfse, avisos)) => {
                    tracing::info!(dps = reserva.id, chave = %nfse.chave, "emissão: autorizada");
                    Ok(ResultadoEmissao::Autorizada { nfse, avisos })
                }
                // A nota existe na Sefin: fica incerta para a reconciliação buscá-la de novo.
                Err(e) => {
                    let motivo = format!(
                        "a Sefin autorizou a NFS-e {}, mas ela não foi gravada: {e}",
                        nfse.chave_acesso
                    );
                    self.banco.registrar_incerta(reserva.id, &motivo).await?;
                    Ok(ResultadoEmissao::Incerta {
                        dps_id: reserva.id,
                        motivo,
                    })
                }
            },
            Err(ErroSefin::Rejeitada { erros, .. }) => {
                let mensagem = mensagem_rejeicao(&erros);
                tracing::warn!(dps = reserva.id, %mensagem, "emissão: rejeitada pela Sefin");
                self.banco.registrar_rejeicao(reserva.id, &mensagem).await?;
                Err(ErroEmissao::Rejeitada {
                    dps_id: Some(reserva.id),
                    mensagem,
                })
            }
            // Não chegou a ser processada: pode tentar de novo.
            Err(e @ (ErroSefin::SemConexao(_) | ErroSefin::LimiteRequisicoes | ErroSefin::Identidade(_))) => {
                let motivo = e.to_string();
                tracing::warn!(dps = reserva.id, %motivo, "emissão: não enviada");
                self.banco
                    .registrar_rejeicao(reserva.id, &format!("não enviada: {motivo}"))
                    .await?;
                Err(ErroEmissao::SemConexao { motivo })
            }
            // Pode ter sido processada (timeout, 5xx, resposta ilegível): reconciliar antes de qualquer coisa.
            Err(e) => {
                let motivo = e.to_string();
                tracing::warn!(dps = reserva.id, %motivo, "emissão: resposta incerta, consultando a Sefin");
                self.banco.registrar_incerta(reserva.id, &motivo).await?;
                match self.reconciliar_com(&sefin, reserva.id).await {
                    Ok(Some(nfse)) => Ok(ResultadoEmissao::Autorizada { nfse, avisos: vec![] }),
                    Ok(None) => Err(ErroEmissao::SemConexao {
                        motivo: format!("{motivo}; a Sefin confirmou que não gerou a nota"),
                    }),
                    Err(_) => Ok(ResultadoEmissao::Incerta {
                        dps_id: reserva.id,
                        motivo,
                    }),
                }
            }
        }
    }

    /// Guarda o corpo da rejeição (ou o texto do erro) da Sefin para o diagnóstico da nota. Falhar aqui
    /// não pode atrapalhar a emissão: só vai para o log.
    async fn registrar_resposta(&self, dps_id: i64, resultado: &Result<nfse_nacional::NfseAutorizada, ErroSefin>) {
        let texto = match resultado {
            Ok(_) => return,
            Err(ErroSefin::Rejeitada { status, corpo, .. }) => format!(
                "HTTP {status}
{corpo}"
            ),
            Err(ErroSefin::Http { status, corpo }) => format!(
                "HTTP {status}
{corpo}"
            ),
            Err(e) => e.to_string(),
        };
        if let Err(e) = self.banco.guardar_resposta_sefin(dps_id, &texto).await {
            tracing::warn!(dps = dps_id, erro = %e, "não foi possível guardar a resposta da Sefin");
        }
    }

    /// Grava a NFS-e no banco e depois confere a assinatura da Sefin e arquiva o XML na pasta.
    /// Erro só antes da gravação; o que falhar depois vira aviso (a nota já está registrada).
    async fn gravar(&self, dps_id: i64, chave: &str, xml: &str) -> Result<(NfseGravada, Vec<String>), ErroEmissao> {
        let empresa_id = self.banco.dps(dps_id).await?.empresa_id;
        let dados = nfse_xml::ler_nfse(xml)?;
        if dados.chave != chave {
            return Err(ErroEmissao::Regra(format!(
                "a chave do XML ({}) não confere com a da resposta ({chave})",
                dados.chave
            )));
        }
        let competencia = dados
            .competencia
            .as_deref()
            .and_then(|c| NaiveDate::parse_from_str(c, "%Y-%m-%d").ok())
            .ok_or_else(|| ErroEmissao::Regra("NFS-e sem data de competência".into()))?;
        let valor = dados
            .valor_servico
            .as_deref()
            .and_then(|v| Valor::parse(v).ok())
            .ok_or_else(|| ErroEmissao::Regra("NFS-e sem valor do serviço".into()))?;
        let recebida = NfseRecebida {
            chave: dados.chave,
            numero: Some(dados.numero),
            xml: xml.to_owned(),
            valor_servico: valor,
            competencia,
        };
        let gravada = self.banco.registrar_autorizacao(dps_id, &recebida).await?;

        let mut avisos = Vec::new();
        if let Err(e) = nfse_xml::verificar(xml) {
            avisos.push(format!("a assinatura da Sefin na NFS-e {chave} não conferiu: {e}"));
        }
        if let Some(raiz) = &self.pasta {
            match self.banco.empresa(empresa_id).await {
                Err(e) => avisos.push(format!("a nota foi autorizada, mas não foi arquivada na pasta: {e}")),
                Ok(empresa) => {
                    avisos.extend(self.arquivar(raiz, &empresa, competencia, &gravada).await);
                    avisos.extend(self.atualizar_pdf_da_substituida(raiz, &empresa, dps_id).await);
                }
            }
        }
        for aviso in &avisos {
            // A nota já está gravada: falha na auditoria não pode virar erro da emissão.
            let _ = self.banco.auditar("aviso", Some(empresa_id), Some(aviso)).await;
        }
        Ok((gravada, avisos))
    }

    /// Grava o XML e o DANFSe na pasta do mês. Devolve o aviso se algo falhar.
    async fn arquivar(
        &self,
        raiz: &std::path::Path,
        empresa: &Empresa,
        competencia: NaiveDate,
        gravada: &NfseGravada,
    ) -> Option<String> {
        let caminho = arquivo::pasta_mes(raiz, empresa, competencia).join(arquivo::nome_xml_nfse(
            gravada.numero.as_deref().unwrap_or("sem-numero"),
            &gravada.chave,
        ));
        if let Err(e) = arquivo::gravar(&caminho, &gravada.xml) {
            Some(format!(
                "a nota foi autorizada, mas não foi gravada em {}: {e}",
                caminho.display()
            ))
        } else if let Err(e) = self
            .gravar_pdf(&caminho, &gravada.xml, nfse_pdf::Situacao::Normal)
            .await
        {
            Some(format!("a nota foi autorizada, mas o PDF não foi gerado: {e}"))
        } else {
            None
        }
    }

    /// Emite a recorrência no mês de referência (o dia é ignorado).
    pub async fn emitir_recorrencia(
        &self,
        recorrencia_id: i64,
        mes: NaiveDate,
    ) -> Result<ResultadoEmissao, ErroEmissao> {
        let item = self.banco.recorrencia(recorrencia_id).await?;
        if !item.servico_ativo {
            return Err(ErroEmissao::Regra(format!(
                "o serviço \"{}\" foi removido do cadastro; edite a recorrência",
                item.servico
            )));
        }
        let r = item.recorrencia;
        let valor = item
            .valor_efetivo
            .ok_or_else(|| ErroEmissao::Regra("recorrência sem valor".into()))?;
        self.emitir(&PedidoEmissao {
            empresa_id: r.empresa_id,
            tomador: r.tomador,
            servico_id: r.servico_id,
            valor,
            descricao: r.descricao,
            mes_referencia: Some(mes.with_day(1).expect("dia 1 existe")),
            obra: r.obra,
            local_prestacao: None,
            informacoes_complementares: r.informacoes_complementares,
            reter: Default::default(),
            origem: Origem::Recorrencia(recorrencia_id),
            substituir: None,
        })
        .await
    }

    /// Emite uma linha de planilha já resolvida (vincula o tomador à empresa, se preciso).
    pub async fn emitir_importado(
        &self,
        item: &importacao::ItemImportado,
        arquivo: Option<&str>,
    ) -> Result<ResultadoEmissao, ErroEmissao> {
        if let Some(erro) = item.erros.first() {
            return Err(ErroEmissao::Regra(format!("linha {}: {erro}", item.linha)));
        }
        let pedido = item
            .pedido(arquivo)
            .ok_or_else(|| ErroEmissao::Regra(format!("linha {} incompleta", item.linha)))?;
        if item.vincular_tomador
            && let Some(pessoa) = item.pessoa_id
        {
            self.banco.vincular_tomador(pedido.empresa_id, pessoa).await?;
        }
        self.emitir(&pedido).await
    }
}

#[cfg(test)]
mod testes;

/// A assinatura (C14N + RSA) é CPU-bound: roda fora das threads assíncronas do Tokio.
async fn assinar_em_thread(xml: String, id: String, cert: Arc<CertificadoA1>) -> Result<String, ErroEmissao> {
    match tokio::task::spawn_blocking(move || nfse_xml::assinar(&xml, &id, cert.as_ref())).await {
        Ok(assinado) => Ok(assinado?),
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(e) => Err(ErroEmissao::Regra(format!("a assinatura foi interrompida: {e}"))),
    }
}
