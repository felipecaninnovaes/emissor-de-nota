//! Emissão confiável: junta banco, geração/assinatura do XML e Sefin.
//!
//! Garantias:
//! - nenhum número é gasto com DPS que não passa na validação local;
//! - a DPS é gravada como `enviando` antes da chamada, então uma queda no meio é retomada
//!   por [`Emissor::reconciliar_pendentes`];
//! - em resposta incerta, nada é reenviado antes de consultar a Sefin pelo Id da DPS;
//! - a NFS-e é gravada exatamente como a Sefin devolveu.

pub mod arquivo;

use std::path::PathBuf;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Utc};

use nfse_cert::CertificadoA1;
use nfse_core::modelo::{Ambiente, Documento, Prestador, Servico as ServicoDps, Valores};
use nfse_core::{Dps, Valor};
use nfse_db::dps::{NfseGravada, NfseRecebida, SituacaoDps};
use nfse_db::empresas::Empresa;
use nfse_db::servicos::Servico;
use nfse_db::tomadores::TomadorVinculado;
use nfse_db::{Banco, Cofre, ErroBanco};
use nfse_nacional::{ClienteSefin, ErroSefin, MensagemSefin, Sefin};
use nfse_xml::{MotivoCancelamento, PedidoCancelamento};

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
    pub informacoes_complementares: Option<String>,
}

/// Brasília (sem horário de verão desde 2019), um minuto atrás: `dhEmi` no futuro é rejeitado.
pub fn agora_brasilia() -> DateTime<FixedOffset> {
    let brasilia = FixedOffset::west_opt(3 * 3600).expect("fuso fixo");
    let instante = Utc::now() - chrono::Duration::minutes(1);
    instante.with_timezone(&brasilia).with_nanosecond_zero()
}

trait SemNanos {
    fn with_nanosecond_zero(self) -> Self;
}

impl SemNanos for DateTime<FixedOffset> {
    fn with_nanosecond_zero(self) -> Self {
        use chrono::Timelike;
        self.with_nanosecond(0).expect("nanossegundo zero é sempre válido")
    }
}

/// Troca `{mes_ref}` por `MM/AAAA`.
pub fn aplicar_mes_ref(texto: &str, mes: NaiveDate) -> String {
    texto.replace("{mes_ref}", &format!("{:02}/{}", mes.month(), mes.year()))
}

fn mes_anterior(data: NaiveDate) -> NaiveDate {
    let primeiro = data.with_day(1).expect("dia 1 existe");
    primeiro
        .pred_opt()
        .expect("data válida")
        .with_day(1)
        .expect("dia 1 existe")
}

/// Monta o modelo da DPS a partir dos cadastros. O número é provisório até a reserva.
pub fn montar_dps(
    empresa: &Empresa,
    tomador: Option<&TomadorVinculado>,
    servico: &Servico,
    pedido: &PedidoEmissao,
    emissao: DateTime<FixedOffset>,
) -> Dps {
    let mes = pedido
        .mes_referencia
        .unwrap_or_else(|| mes_anterior(emissao.date_naive()));
    let descricao = aplicar_mes_ref(pedido.descricao.as_deref().unwrap_or(&servico.descricao), mes);
    let t = &servico.tributacao;
    Dps {
        ambiente: empresa.ambiente,
        emissao,
        versao_aplicativo: VERSAO_APLICATIVO.into(),
        serie: empresa.serie.clone(),
        numero: 1,
        // Competência = data da emissão; o mês de referência vai só na descrição.
        competencia: emissao.date_naive(),
        municipio_emissor: empresa.municipio_emissor.clone(),
        substituicao: None,
        prestador: Prestador {
            documento: Documento::Cnpj(empresa.cnpj.clone()),
            inscricao_municipal: empresa.im_para_dps(empresa.ambiente).map(str::to_owned),
            telefone: empresa.telefone.clone(),
            email: empresa.email.clone(),
            regime: empresa.regime.clone(),
        },
        tomador: tomador.map(|t| t.tomador.clone()),
        servico: ServicoDps {
            municipio_prestacao: empresa.municipio_emissor.clone(),
            codigo_tributacao_nacional: servico.codigo_trib_nac.clone(),
            codigo_tributacao_municipal: servico.codigo_trib_mun.clone(),
            descricao,
            nbs: servico.nbs.clone(),
            informacoes_complementares: pedido.informacoes_complementares.clone(),
        },
        valores: Valores {
            servico: pedido.valor,
            desconto_incondicionado: None,
            desconto_condicionado: None,
            issqn: t.issqn.clone(),
            federais: t.federais.clone(),
            total_tributos: t.total_tributos.clone(),
        },
        ibs_cbs: t.ibs_cbs.clone(),
    }
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
}

impl<C: Conector> Emissor<C> {
    pub fn novo(banco: Banco, cofre: Cofre, conector: C) -> Self {
        Self {
            banco,
            cofre,
            conector,
            pasta: None,
        }
    }

    /// Pasta raiz onde cada nota é gravada em `<empresa>/<AAAA-MM>/`.
    pub fn com_pasta(mut self, raiz: impl Into<PathBuf>) -> Self {
        self.pasta = Some(raiz.into());
        self
    }

    pub fn banco(&self) -> &Banco {
        &self.banco
    }

    async fn conectar(&self, empresa: &Empresa) -> Result<(CertificadoA1, C::S), ErroEmissao> {
        let cert = self.banco.certificado_ativo(&self.cofre, empresa.id).await?;
        let sefin = self
            .conector
            .conectar(&cert, empresa.ambiente)
            .map_err(ErroEmissao::Sefin)?;
        Ok((cert, sefin))
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

        // Valida antes de reservar número: DPS inválida não gasta numeração.
        let mut modelo = montar_dps(&empresa, tomador.as_ref(), &servico, pedido, agora_brasilia());
        nfse_xml::gerar_dps(&modelo)?;
        let (cert, sefin) = self.conectar(&empresa).await?;

        let reserva = self.banco.reservar_dps(empresa.id).await?;
        modelo.serie = reserva.serie.clone();
        modelo.numero = reserva.numero;
        let (xml, id) = nfse_xml::gerar_dps(&modelo)?;
        let assinado = nfse_xml::assinar(&xml, id.as_str(), &cert)?;
        self.banco.registrar_envio(reserva.id, &modelo, &assinado).await?;

        match sefin.emitir(&assinado).await {
            Ok(nfse) => {
                let (nfse, avisos) = self.gravar(reserva.id, &nfse.chave_acesso, &nfse.xml).await?;
                Ok(ResultadoEmissao::Autorizada { nfse, avisos })
            }
            Err(ErroSefin::Rejeitada { erros, .. }) => {
                let mensagem = mensagem_rejeicao(&erros);
                self.banco.registrar_rejeicao(reserva.id, &mensagem).await?;
                Err(ErroEmissao::Rejeitada {
                    dps_id: Some(reserva.id),
                    mensagem,
                })
            }
            // Não chegou a ser processada: pode tentar de novo.
            Err(e @ (ErroSefin::SemConexao(_) | ErroSefin::LimiteRequisicoes | ErroSefin::Identidade(_))) => {
                let motivo = e.to_string();
                self.banco
                    .registrar_rejeicao(reserva.id, &format!("não enviada: {motivo}"))
                    .await?;
                Err(ErroEmissao::SemConexao { motivo })
            }
            // Pode ter sido processada (timeout, 5xx, resposta ilegível): reconciliar antes de qualquer coisa.
            Err(e) => {
                let motivo = e.to_string();
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

    /// Grava a NFS-e no banco e depois confere a assinatura da Sefin e arquiva o XML na pasta.
    async fn gravar(&self, dps_id: i64, chave: &str, xml: &str) -> Result<(NfseGravada, Vec<String>), ErroEmissao> {
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

        let empresa_id = self.banco.dps(dps_id).await?.empresa_id;
        let mut avisos = Vec::new();
        if let Err(e) = nfse_xml::verificar(xml) {
            avisos.push(format!("a assinatura da Sefin na NFS-e {chave} não conferiu: {e}"));
        }
        if let Some(raiz) = &self.pasta {
            let empresa = self.banco.empresa(empresa_id).await?;
            let caminho = arquivo::pasta_mes(raiz, &empresa, competencia).join(arquivo::nome_xml_nfse(
                gravada.numero.as_deref().unwrap_or("sem-numero"),
                chave,
            ));
            if let Err(e) = arquivo::gravar(&caminho, xml) {
                avisos.push(format!(
                    "a nota foi autorizada, mas não foi gravada em {}: {e}",
                    caminho.display()
                ));
            }
        }
        for aviso in &avisos {
            self.banco.auditar("aviso", Some(empresa_id), Some(aviso)).await?;
        }
        Ok((gravada, avisos))
    }

    /// Consulta a Sefin pelo Id da DPS: grava a NFS-e se existir, ou libera para reenvio se não.
    pub async fn reconciliar(&self, dps_id: i64) -> Result<Option<NfseGravada>, ErroEmissao> {
        let dps = self.banco.dps(dps_id).await?;
        let empresa = self.banco.empresa(dps.empresa_id).await?;
        let (_, sefin) = self.conectar(&empresa).await?;
        self.reconciliar_com(&sefin, dps_id).await
    }

    async fn reconciliar_com(&self, sefin: &C::S, dps_id: i64) -> Result<Option<NfseGravada>, ErroEmissao> {
        let dps = self.banco.dps(dps_id).await?;
        if !matches!(dps.situacao, SituacaoDps::Enviando | SituacaoDps::Incerta) {
            return Err(ErroEmissao::Regra(format!(
                "DPS em situação \"{}\" não precisa de reconciliação",
                dps.situacao.texto()
            )));
        }
        match sefin.consultar_dps(&dps.id_dps).await.map_err(ErroEmissao::Sefin)? {
            Some(chave) => {
                let nfse = sefin.consultar_nfse(&chave).await.map_err(ErroEmissao::Sefin)?;
                Ok(Some(self.gravar(dps_id, &nfse.chave_acesso, &nfse.xml).await?.0))
            }
            None => {
                self.banco.liberar_para_reenvio(dps_id).await?;
                Ok(None)
            }
        }
    }

    /// Ao abrir o sistema: resolve DPS que ficaram sem desfecho. Devolve o resultado de cada uma.
    pub async fn reconciliar_pendentes(
        &self,
    ) -> Result<Vec<(i64, Result<Option<NfseGravada>, ErroEmissao>)>, ErroEmissao> {
        let mut resultados = Vec::new();
        for dps in self.banco.dps_pendentes().await? {
            if dps.situacao == SituacaoDps::Enviando {
                self.banco
                    .registrar_incerta(dps.id, "o programa foi fechado durante o envio")
                    .await?;
            }
            resultados.push((dps.id, self.reconciliar(dps.id).await));
        }
        Ok(resultados)
    }

    pub async fn cancelar(
        &self,
        nfse_id: i64,
        motivo: MotivoCancelamento,
        justificativa: &str,
    ) -> Result<(), ErroEmissao> {
        let nfse = self.banco.nfse(nfse_id).await?;
        let dps = self.banco.dps(nfse.dps_id).await?;
        if dps.situacao != SituacaoDps::Autorizada {
            return Err(ErroEmissao::Regra(format!(
                "a nota está \"{}\" e não pode ser cancelada",
                dps.situacao.texto()
            )));
        }
        let empresa = self.banco.empresa(dps.empresa_id).await?;
        let (cert, sefin) = self.conectar(&empresa).await?;
        let pedido = PedidoCancelamento {
            ambiente: dps.ambiente,
            versao_aplicativo: VERSAO_APLICATIVO.into(),
            quando: agora_brasilia(),
            autor: Documento::Cnpj(empresa.cnpj.clone()),
            chave: nfse.chave.clone(),
            motivo,
            descricao: justificativa.to_owned(),
        };
        let (xml, id) = nfse_xml::gerar_pedido_cancelamento(&pedido)?;
        let assinado = nfse_xml::assinar(&xml, &id, &cert)?;
        // O Id do pedido é fixo por chave e tipo de evento: repetir após falha não duplica.
        let evento = sefin
            .registrar_evento(&nfse.chave, &assinado)
            .await
            .map_err(|e| match e {
                ErroSefin::Rejeitada { erros, .. } => ErroEmissao::Rejeitada {
                    dps_id: None,
                    mensagem: mensagem_rejeicao(&erros),
                },
                outro => ErroEmissao::Sefin(outro),
            })?;
        self.banco.registrar_cancelamento(nfse_id, &evento).await?;

        if let Some(raiz) = &self.pasta {
            let competencia = nfse_xml::ler_nfse(&nfse.xml)?
                .competencia
                .and_then(|c| NaiveDate::parse_from_str(&c, "%Y-%m-%d").ok())
                .unwrap_or_else(|| agora_brasilia().date_naive());
            let caminho = arquivo::pasta_mes(raiz, &empresa, competencia).join(arquivo::nome_xml_evento(
                nfse.numero.as_deref().unwrap_or("sem-numero"),
                &nfse.chave,
                "cancelamento",
            ));
            if let Err(e) = arquivo::gravar(&caminho, &evento) {
                let aviso = format!("cancelamento registrado, mas não gravado em {}: {e}", caminho.display());
                self.banco.auditar("aviso", Some(empresa.id), Some(&aviso)).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes;
