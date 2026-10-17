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

/// Dados de uma DPS rejeitada para preencher uma nova emissão (que recebe o próximo número:
/// pelo Anexo I, só a DPS que gerou NFS-e trava série e número — regra E0014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refazer {
    pub empresa_id: i64,
    /// `None` se não havia tomador ou se ele não está mais vinculado à empresa.
    pub tomador: Option<TomadorVinculado>,
    /// Serviço do cadastro com o mesmo código (e NBS, se houver mais de um); o operador confere.
    pub servico_id: Option<i64>,
    pub valor: Valor,
    /// Descrição como foi enviada (com o mês de referência já aplicado).
    pub descricao: String,
    pub informacoes_complementares: Option<String>,
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
    pub origem: Origem,
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

/// Tributos federais da nota: os percentuais de retenção do serviço viram valores sobre o valor da nota.
pub fn federais_da_nota(t: &nfse_db::servicos::Tributacao, valor: Valor) -> nfse_core::modelo::TributosFederais {
    let r = &t.retencoes;
    let mut f = t.federais.clone();
    let aplicar = |p: Option<nfse_core::Percentual>| p.map(|p| valor.aplicar(p));
    f.retencao_irrf = aplicar(r.irrf).or(f.retencao_irrf);
    f.retencao_csll = aplicar(r.csll).or(f.retencao_csll);
    f.retencao_cp = aplicar(r.cp).or(f.retencao_cp);
    if r.pis.is_some() || r.cofins.is_some() {
        let pc = f.pis_cofins.get_or_insert_with(|| nfse_core::modelo::PisCofins {
            cst: "01".into(),
            base_calculo: None,
            aliquota_pis: None,
            aliquota_cofins: None,
            valor_pis: None,
            valor_cofins: None,
            tipo_retencao: None,
        });
        pc.base_calculo = Some(valor);
        pc.aliquota_pis = r.pis;
        pc.aliquota_cofins = r.cofins;
        pc.valor_pis = aplicar(r.pis);
        pc.valor_cofins = aplicar(r.cofins);
    }
    f
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
            federais: federais_da_nota(t, pedido.valor),
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
    /// Fontes do DANFSe (carregadas junto com a pasta; falha vira aviso na hora de gerar o PDF).
    fontes: Option<Result<nfse_pdf::Fontes, String>>,
    logo: Option<Vec<u8>>,
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
        }
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
    fn gerar_pdf(&self, xml: &str, situacao: nfse_pdf::Situacao) -> Result<Option<Vec<u8>>, String> {
        let fontes = match &self.fontes {
            Some(Ok(f)) => f,
            Some(Err(e)) => return Err(e.clone()),
            None => return Ok(None),
        };
        let opcoes = nfse_pdf::Opcoes {
            situacao,
            logo_png: self.logo.clone(),
        };
        nfse_pdf::gerar_danfse(xml, &opcoes, fontes)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// Gera o DANFSe e grava ao lado do XML (mesmo nome, extensão .pdf).
    fn gravar_pdf(&self, caminho_xml: &std::path::Path, xml: &str, situacao: nfse_pdf::Situacao) -> Result<(), String> {
        let Some(pdf) = self.gerar_pdf(xml, situacao)? else {
            return Ok(());
        };
        let caminho = caminho_xml.with_extension("pdf");
        arquivo::gravar(&caminho, &pdf).map_err(|e| format!("{}: {e}", caminho.display()))
    }

    pub fn banco(&self) -> &Banco {
        &self.banco
    }

    /// O ambiente vem da DPS quando ela já existe: a empresa pode ter mudado de ambiente depois.
    async fn conectar(&self, empresa: &Empresa, ambiente: Ambiente) -> Result<(CertificadoA1, C::S), ErroEmissao> {
        let cert = self.banco.certificado_ativo(&self.cofre, empresa.id).await?;
        let sefin = self.conector.conectar(&cert, ambiente).map_err(ErroEmissao::Sefin)?;
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
        nfse_xml::gerar_dps(&modelo)?;
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
            } else if let Err(e) = self.gravar_pdf(&caminho, xml, nfse_pdf::Situacao::Normal) {
                avisos.push(format!("a nota foi autorizada, mas o PDF não foi gerado: {e}"));
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
        let (_, sefin) = self.conectar(&empresa, dps.ambiente).await?;
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

    /// Procura na Sefin o primeiro número da série atual da empresa que ainda não gerou NFS-e
    /// (outro emissor pode ter usado a mesma série: rejeição E0014) e o grava como próximo.
    /// Usa `GET /dps/{id}` em busca exponencial a partir do próximo número local e depois binária,
    /// supondo que o outro emissor numerou em sequência. Devolve (último usado, próximo).
    pub async fn ajustar_numeracao_pela_sefin(&self, empresa_id: i64) -> Result<(Option<u64>, u64), ErroEmissao> {
        const LIMITE: u64 = 999_999_999_999_999;
        let empresa = self.banco.empresa(empresa_id).await?;
        let (_, sefin) = self.conectar(&empresa, empresa.ambiente).await?;
        let inscricao = nfse_core::Inscricao::Cnpj(empresa.cnpj.clone());
        let usado = |numero: u64| {
            let id = nfse_core::dps::DpsId::novo(&empresa.municipio_emissor, &inscricao, &empresa.serie, numero);
            let sefin = &sefin;
            async move {
                let id = id.map_err(|e| ErroEmissao::Regra(e.to_string()))?;
                Ok::<_, ErroEmissao>(
                    sefin
                        .consultar_dps(id.as_str())
                        .await
                        .map_err(ErroEmissao::Sefin)?
                        .is_some(),
                )
            }
        };

        let inicio = self
            .banco
            .proximo_numero(empresa.id, empresa.ambiente, &empresa.serie)
            .await?;
        // Exponencial: `ocupado` é o maior número que sabemos usado; `livre`, um que sabemos livre.
        let mut ocupado = inicio - 1;
        let mut passo = 1;
        let mut livre = inicio;
        while usado(livre).await? {
            ocupado = livre;
            livre = inicio.saturating_add(passo).min(LIMITE);
            if livre == ocupado {
                return Err(ErroEmissao::Regra("a série não tem mais números livres".into()));
            }
            passo *= 2;
        }
        // Binária entre os dois.
        while livre - ocupado > 1 {
            let meio = ocupado + (livre - ocupado) / 2;
            if usado(meio).await? {
                ocupado = meio;
            } else {
                livre = meio;
            }
        }
        if livre != inicio {
            self.banco
                .definir_proximo_numero(empresa.id, empresa.ambiente, &empresa.serie, livre)
                .await?;
        }
        Ok(((ocupado > 0).then_some(ocupado), livre))
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
            informacoes_complementares: r.informacoes_complementares,
            origem: Origem::Recorrencia(recorrencia_id),
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

    /// Envia a NFS-e (PDF e XML) por e-mail. `para` vazio usa os destinatários do cadastro do
    /// tomador. Cada tentativa fica registrada. Devolve para quem foi.
    pub async fn enviar_email(
        &self,
        nfse_id: i64,
        para: &[String],
        carteiro: &impl email::Carteiro,
    ) -> Result<Vec<String>, ErroEmissao> {
        let cfg = self
            .banco
            .smtp()
            .await?
            .ok_or_else(|| ErroEmissao::Regra("configure o envio de e-mail em Configurações".into()))?;
        let senha = self.banco.senha_smtp(&self.cofre).await?;
        let dados = self.banco.dados_email_nfse(nfse_id).await?;
        let destinatarios = if para.is_empty() {
            dados.destinatarios.clone()
        } else {
            para.to_vec()
        };
        let situacao = if dados.cancelada {
            nfse_pdf::Situacao::Cancelada
        } else {
            nfse_pdf::Situacao::Normal
        };
        // Sem PDF (fontes indisponíveis), vai só o XML, que é o documento fiscal.
        let pdf = self.gerar_pdf(&dados.xml, situacao).ok().flatten();
        let (mensagem, assunto) = match email::montar(&cfg, &dados, &destinatarios, pdf) {
            Ok(m) => m,
            Err(e) => {
                self.banco
                    .registrar_envio_email(nfse_id, &destinatarios, &cfg.assunto, Some(&e))
                    .await?;
                return Err(ErroEmissao::Regra(e));
            }
        };
        let resultado = carteiro
            .entregar(&cfg, senha.as_deref().map(String::as_str), mensagem)
            .await;
        self.banco
            .registrar_envio_email(
                nfse_id,
                &destinatarios,
                &assunto,
                resultado.as_ref().err().map(String::as_str),
            )
            .await?;
        resultado.map_err(ErroEmissao::Regra)?;
        self.banco
            .auditar("email_enviado", Some(dados.empresa_id), Some(&destinatarios.join("; ")))
            .await?;
        Ok(destinatarios)
    }

    /// Envia uma mensagem de teste com a configuração gravada.
    pub async fn enviar_email_teste(&self, para: &str, carteiro: &impl email::Carteiro) -> Result<(), ErroEmissao> {
        let cfg = self
            .banco
            .smtp()
            .await?
            .ok_or_else(|| ErroEmissao::Regra("salve a configuração antes de testar".into()))?;
        let senha = self.banco.senha_smtp(&self.cofre).await?;
        let mensagem = email::mensagem_teste(&cfg, para).map_err(ErroEmissao::Regra)?;
        carteiro
            .entregar(&cfg, senha.as_deref().map(String::as_str), mensagem)
            .await
            .map_err(ErroEmissao::Regra)
    }

    /// Depois de uma emissão autorizada: envia se a opção estiver ligada. Devolve o que dizer ao
    /// operador (`None` = envio automático desligado ou e-mail não configurado).
    pub async fn enviar_email_apos_emissao(&self, nfse_id: i64, carteiro: &impl email::Carteiro) -> Option<String> {
        let cfg = self.banco.smtp().await.ok().flatten()?;
        if !cfg.enviar_ao_emitir {
            return None;
        }
        Some(match self.enviar_email(nfse_id, &[], carteiro).await {
            Ok(para) => format!("E-mail enviado para {}.", para.join(", ")),
            Err(e) => format!("E-mail não enviado: {e}. Reenvie em \"Notas emitidas\"."),
        })
    }

    /// Lê uma DPS rejeitada (pela Sefin ou não enviada por falta de conexão) para emitir de novo.
    pub async fn dados_para_refazer(&self, dps_id: i64) -> Result<Refazer, ErroEmissao> {
        let registro = self.banco.dps(dps_id).await?;
        if registro.situacao != SituacaoDps::Rejeitada {
            return Err(ErroEmissao::Regra(format!(
                "só nota rejeitada pode ser refeita (esta está \"{}\")",
                registro.situacao.texto()
            )));
        }
        let modelo = self
            .banco
            .modelo_dps(dps_id)
            .await?
            .ok_or_else(|| ErroEmissao::Regra("a DPS não tem os dados do envio".into()))?;
        let empresa_id = registro.empresa_id;

        let mut tomador = None;
        if let Some(t) = &modelo.tomador {
            let documento = match &t.documento {
                Documento::Cnpj(c) => c.as_str(),
                Documento::Cpf(c) => c.as_str(),
            };
            if let Some(pessoa) = self.banco.pessoa_por_documento(documento).await? {
                tomador = self.banco.tomador(empresa_id, pessoa).await.ok();
            }
        }

        let codigo = &modelo.servico.codigo_tributacao_nacional;
        let candidatos: Vec<Servico> = self
            .banco
            .servicos(empresa_id)
            .await?
            .into_iter()
            .filter(|s| &s.codigo_trib_nac == codigo)
            .collect();
        let servico_id = candidatos
            .iter()
            .find(|s| s.nbs == modelo.servico.nbs)
            .or(candidatos.first())
            .and_then(|s| s.id);

        Ok(Refazer {
            empresa_id,
            tomador,
            servico_id,
            valor: modelo.valores.servico,
            descricao: modelo.servico.descricao,
            informacoes_complementares: modelo.servico.informacoes_complementares,
        })
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
        let (cert, sefin) = self.conectar(&empresa, dps.ambiente).await?;
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
            // O PDF da nota passa a ter a marca d'água CANCELADA.
            let caminho_xml = arquivo::pasta_mes(raiz, &empresa, competencia).join(arquivo::nome_xml_nfse(
                nfse.numero.as_deref().unwrap_or("sem-numero"),
                &nfse.chave,
            ));
            if let Err(e) = self.gravar_pdf(&caminho_xml, &nfse.xml, nfse_pdf::Situacao::Cancelada) {
                let aviso = format!("cancelamento registrado, mas o PDF não foi atualizado: {e}");
                self.banco.auditar("aviso", Some(empresa.id), Some(&aviso)).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes;
