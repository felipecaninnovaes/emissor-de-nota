//! Comandos chamados pela interface. Erros voltam como texto pronto para mostrar ao operador.

use std::path::PathBuf;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::modelo::{Ambiente, RegimeTributario, Tomador};
use nfse_core::{Cnpj, Valor};
use nfse_db::dps::SituacaoDps;
use nfse_db::empresas::NovaEmpresa;
use nfse_db::servicos::{Servico, Tributacao};
use nfse_db::tomadores::TomadorVinculado;
use nfse_emissor::{ConectorSefin, Emissor, PedidoEmissao, ResultadoEmissao};
use nfse_xml::MotivoCancelamento;

use crate::Estado;

type R<T> = Result<T, String>;

fn texto<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const CHAVE_PASTA: &str = "pasta_saida";

/// Pasta padrão: Documentos\NFS-e do usuário.
fn pasta_padrao() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Documents").join("NFS-e")
}

async fn pasta_configurada(estado: &Estado) -> R<PathBuf> {
    Ok(estado
        .banco
        .configuracao(CHAVE_PASTA)
        .await
        .map_err(texto)?
        .map(PathBuf::from)
        .unwrap_or_else(pasta_padrao))
}

macro_rules! emissor {
    ($estado:expr) => {{
        let guarda = $estado.emissor.read().await;
        if guarda.is_none() {
            return Err("abra o sistema com a senha mestra".into());
        }
        guarda
    }};
}

// ---------- cofre e configuração ----------

#[derive(Serialize)]
pub struct SituacaoCofre {
    existe: bool,
    aberto: bool,
}

#[tauri::command]
pub async fn situacao_cofre(estado: State<'_, Estado>) -> R<SituacaoCofre> {
    Ok(SituacaoCofre {
        existe: estado.banco.cofre_existe().await.map_err(texto)?,
        aberto: estado.emissor.read().await.is_some(),
    })
}

#[derive(Serialize)]
pub struct Abertura {
    /// Resultado da reconciliação das DPS que ficaram sem desfecho.
    pendencias: Vec<String>,
}

async fn iniciar(estado: &Estado, cofre: nfse_db::Cofre) -> R<Abertura> {
    let pasta = pasta_configurada(estado).await?;
    let emissor = Emissor::novo(estado.banco.clone(), cofre, ConectorSefin).com_pasta(pasta);
    let mut pendencias = Vec::new();
    for (dps_id, resultado) in emissor.reconciliar_pendentes().await.map_err(texto)? {
        pendencias.push(match resultado {
            Ok(Some(n)) => format!(
                "DPS {dps_id}: a Sefin tinha autorizado a nota {}; gravada.",
                n.numero.unwrap_or_default()
            ),
            Ok(None) => format!("DPS {dps_id}: a Sefin não gerou nota; pode emitir de novo."),
            Err(e) => format!("DPS {dps_id}: ainda sem resposta ({e}); será verificada na próxima abertura."),
        });
    }
    *estado.emissor.write().await = Some(emissor);
    Ok(Abertura { pendencias })
}

#[tauri::command]
pub async fn criar_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let cofre = estado.banco.criar_cofre(&senha).await.map_err(texto)?;
    iniciar(&estado, cofre).await
}

#[tauri::command]
pub async fn abrir_cofre(estado: State<'_, Estado>, senha: String) -> R<Abertura> {
    let cofre = estado.banco.abrir_cofre(&senha).await.map_err(texto)?;
    iniciar(&estado, cofre).await
}

#[tauri::command]
pub async fn obter_pasta(estado: State<'_, Estado>) -> R<String> {
    Ok(pasta_configurada(&estado).await?.display().to_string())
}

#[tauri::command]
pub async fn definir_pasta(estado: State<'_, Estado>, pasta: String) -> R<()> {
    let caminho = PathBuf::from(pasta.trim());
    std::fs::create_dir_all(&caminho).map_err(|e| format!("não foi possível usar a pasta: {e}"))?;
    estado
        .banco
        .definir_configuracao(CHAVE_PASTA, &caminho.display().to_string())
        .await
        .map_err(texto)?;
    if let Some(emissor) = estado.emissor.write().await.as_mut() {
        emissor.definir_pasta(caminho);
    }
    Ok(())
}

// ---------- empresas e certificados ----------

#[derive(Serialize)]
pub struct EmpresaDto {
    id: i64,
    cnpj: String,
    cnpj_formatado: String,
    nome: String,
    inscricao_municipal: Option<String>,
    im_na_dps_producao: bool,
    im_na_dps_homologacao: bool,
    municipio_emissor: String,
    regime: RegimeTributario,
    ambiente: Ambiente,
    serie: String,
    telefone: Option<String>,
    email: Option<String>,
    certificado: Option<CertificadoDto>,
}

#[derive(Serialize)]
pub struct CertificadoDto {
    titular: String,
    valido_ate: String,
    dias_para_vencer: i64,
}

#[tauri::command]
pub async fn listar_empresas(estado: State<'_, Estado>) -> R<Vec<EmpresaDto>> {
    let mut lista = Vec::new();
    for e in estado.banco.empresas().await.map_err(texto)? {
        let certificado = estado
            .banco
            .certificado_resumo(e.id)
            .await
            .map_err(texto)?
            .map(|(titular, ate)| CertificadoDto {
                titular,
                valido_ate: ate.format("%d/%m/%Y").to_string(),
                dias_para_vencer: (ate - Utc::now()).num_days(),
            });
        lista.push(EmpresaDto {
            id: e.id,
            cnpj: e.cnpj.as_str().to_owned(),
            cnpj_formatado: e.cnpj.to_string(),
            nome: e.nome,
            inscricao_municipal: e.inscricao_municipal,
            im_na_dps_producao: e.im_na_dps_producao,
            im_na_dps_homologacao: e.im_na_dps_homologacao,
            municipio_emissor: e.municipio_emissor,
            regime: e.regime,
            ambiente: e.ambiente,
            serie: e.serie,
            telefone: e.telefone,
            email: e.email,
            certificado,
        });
    }
    Ok(lista)
}

#[derive(Deserialize)]
pub struct NovaEmpresaDto {
    cnpj: String,
    nome: String,
    inscricao_municipal: Option<String>,
    municipio_emissor: String,
    regime: RegimeTributario,
    ambiente: Ambiente,
    serie: String,
    telefone: Option<String>,
    email: Option<String>,
}

fn vazio_para_none(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn cadastrar_empresa(estado: State<'_, Estado>, empresa: NovaEmpresaDto) -> R<i64> {
    let nova = NovaEmpresa {
        cnpj: Cnpj::parse(&empresa.cnpj).map_err(|e| format!("CNPJ: {e}"))?,
        nome: empresa.nome,
        inscricao_municipal: vazio_para_none(empresa.inscricao_municipal),
        municipio_emissor: empresa.municipio_emissor.trim().to_owned(),
        regime: empresa.regime,
        ambiente: empresa.ambiente,
        serie: empresa.serie.trim().to_owned(),
        telefone: vazio_para_none(empresa.telefone).map(|t| t.chars().filter(char::is_ascii_digit).collect()),
        email: vazio_para_none(empresa.email),
    };
    Ok(estado.banco.cadastrar_empresa(&nova).await.map_err(texto)?.id)
}

#[tauri::command]
pub async fn importar_certificado(
    estado: State<'_, Estado>,
    empresa_id: i64,
    pfx: Vec<u8>,
    senha: String,
) -> R<CertificadoDto> {
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    let info = estado
        .banco
        .importar_certificado(cofre, empresa_id, &pfx, &senha)
        .await
        .map_err(texto)?;
    Ok(CertificadoDto {
        dias_para_vencer: info.dias_para_vencer(Utc::now()),
        valido_ate: info.valido_ate.format("%d/%m/%Y").to_string(),
        titular: info.titular,
    })
}

#[tauri::command]
pub async fn definir_im_na_dps(
    estado: State<'_, Estado>,
    empresa_id: i64,
    ambiente: Ambiente,
    informar: bool,
) -> R<()> {
    estado
        .banco
        .definir_im_na_dps(empresa_id, ambiente, informar)
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn definir_proximo_numero(
    estado: State<'_, Estado>,
    empresa_id: i64,
    ambiente: Ambiente,
    serie: String,
    proximo: u64,
) -> R<()> {
    estado
        .banco
        .definir_proximo_numero(empresa_id, ambiente, &serie, proximo)
        .await
        .map_err(texto)
}

#[derive(Serialize)]
pub struct VencimentoDto {
    empresa_id: i64,
    empresa: String,
    valido_ate: String,
    dias: i64,
}

#[tauri::command]
pub async fn certificados_vencendo(estado: State<'_, Estado>, dias: i64) -> R<Vec<VencimentoDto>> {
    Ok(estado
        .banco
        .certificados_vencendo(dias)
        .await
        .map_err(texto)?
        .into_iter()
        .map(|v| VencimentoDto {
            empresa_id: v.empresa_id,
            empresa: v.empresa,
            valido_ate: v.valido_ate.format("%d/%m/%Y").to_string(),
            dias: (v.valido_ate - Utc::now()).num_days(),
        })
        .collect())
}

// ---------- tomadores ----------

#[derive(Serialize)]
pub struct TomadorDto {
    pessoa_id: i64,
    tomador: Tomador,
    emails: Option<String>,
}

impl From<TomadorVinculado> for TomadorDto {
    fn from(t: TomadorVinculado) -> Self {
        Self {
            pessoa_id: t.pessoa_id,
            tomador: t.tomador,
            emails: t.emails,
        }
    }
}

#[tauri::command]
pub async fn buscar_tomadores(estado: State<'_, Estado>, empresa_id: i64, termo: String) -> R<Vec<TomadorDto>> {
    Ok(estado
        .banco
        .buscar_tomadores(empresa_id, &termo)
        .await
        .map_err(texto)?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub async fn salvar_tomador(
    estado: State<'_, Estado>,
    empresa_id: i64,
    tomador: Tomador,
    emails: Option<String>,
) -> R<i64> {
    let id = estado
        .banco
        .salvar_tomador(empresa_id, &tomador, "manual")
        .await
        .map_err(texto)?;
    estado
        .banco
        .definir_emails_tomador(empresa_id, id, vazio_para_none(emails).as_deref(), None)
        .await
        .map_err(texto)?;
    Ok(id)
}

// ---------- serviços ----------

#[derive(Serialize, Deserialize)]
pub struct ServicoDto {
    id: Option<i64>,
    empresa_id: i64,
    nome: String,
    descricao: String,
    codigo_trib_nac: String,
    codigo_trib_mun: Option<String>,
    nbs: Option<String>,
    tributacao: Tributacao,
    valor_sugerido: Option<Valor>,
}

impl From<Servico> for ServicoDto {
    fn from(s: Servico) -> Self {
        Self {
            id: s.id,
            empresa_id: s.empresa_id,
            nome: s.nome,
            descricao: s.descricao,
            codigo_trib_nac: s.codigo_trib_nac,
            codigo_trib_mun: s.codigo_trib_mun,
            nbs: s.nbs,
            tributacao: s.tributacao,
            valor_sugerido: s.valor_sugerido,
        }
    }
}

#[tauri::command]
pub async fn listar_servicos(estado: State<'_, Estado>, empresa_id: i64) -> R<Vec<ServicoDto>> {
    Ok(estado
        .banco
        .servicos(empresa_id)
        .await
        .map_err(texto)?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub async fn salvar_servico(estado: State<'_, Estado>, servico: ServicoDto) -> R<i64> {
    let s = Servico {
        id: servico.id,
        empresa_id: servico.empresa_id,
        nome: servico.nome,
        descricao: servico.descricao,
        codigo_trib_nac: servico.codigo_trib_nac.chars().filter(char::is_ascii_digit).collect(),
        codigo_trib_mun: vazio_para_none(servico.codigo_trib_mun),
        nbs: vazio_para_none(servico.nbs).map(|n| n.chars().filter(char::is_ascii_digit).collect()),
        tributacao: servico.tributacao,
        valor_sugerido: servico.valor_sugerido,
    };
    estado.banco.salvar_servico(&s).await.map_err(texto)
}

#[tauri::command]
pub async fn desativar_servico(estado: State<'_, Estado>, id: i64) -> R<()> {
    estado.banco.desativar_servico(id).await.map_err(texto)
}

// ---------- emissão e notas ----------

#[derive(Deserialize)]
pub struct PedidoDto {
    empresa_id: i64,
    tomador: Option<i64>,
    servico_id: i64,
    valor: Valor,
    descricao: Option<String>,
    /// `AAAA-MM`
    mes_referencia: Option<String>,
    informacoes_complementares: Option<String>,
}

impl PedidoDto {
    fn pedido(self) -> R<PedidoEmissao> {
        let mes_referencia = match vazio_para_none(self.mes_referencia) {
            Some(m) => Some(
                NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").map_err(|_| "mês de referência inválido")?,
            ),
            None => None,
        };
        Ok(PedidoEmissao {
            empresa_id: self.empresa_id,
            tomador: self.tomador,
            servico_id: self.servico_id,
            valor: self.valor,
            descricao: vazio_para_none(self.descricao),
            mes_referencia,
            informacoes_complementares: vazio_para_none(self.informacoes_complementares),
        })
    }
}

#[derive(Serialize)]
pub struct PreviaDto {
    ambiente: Ambiente,
    serie: String,
    prestador: String,
    tomador: Option<String>,
    descricao: String,
    valor: String,
    competencia: String,
    erros: Vec<String>,
}

/// Mostra como a DPS vai sair (descrição com o mês já aplicado) e os erros de validação, sem emitir.
#[tauri::command]
pub async fn previa_dps(estado: State<'_, Estado>, pedido: PedidoDto) -> R<PreviaDto> {
    let pedido = pedido.pedido()?;
    let b = &estado.banco;
    let empresa = b.empresa(pedido.empresa_id).await.map_err(texto)?;
    let servico = b.servico(pedido.servico_id).await.map_err(texto)?;
    let tomador = match pedido.tomador {
        Some(id) => Some(b.tomador(empresa.id, id).await.map_err(texto)?),
        None => None,
    };
    let dps = nfse_emissor::montar_dps(
        &empresa,
        tomador.as_ref(),
        &servico,
        &pedido,
        nfse_emissor::agora_brasilia(),
    );
    let mut erros = dps.validar();
    if b.certificado_resumo(empresa.id).await.map_err(texto)?.is_none() {
        erros.push("a empresa não tem certificado digital importado".into());
    }
    Ok(PreviaDto {
        ambiente: dps.ambiente,
        serie: dps.serie,
        prestador: format!("{} ({})", empresa.nome, empresa.cnpj),
        tomador: dps.tomador.map(|t| t.nome),
        descricao: dps.servico.descricao,
        valor: dps.valores.servico.to_string(),
        competencia: dps.competencia.format("%d/%m/%Y").to_string(),
        erros,
    })
}

#[derive(Serialize)]
pub struct ResultadoDto {
    autorizada: bool,
    nfse_id: Option<i64>,
    numero: Option<String>,
    chave: Option<String>,
    avisos: Vec<String>,
    /// Quando incerta: o que aconteceu.
    motivo: Option<String>,
}

#[tauri::command]
pub async fn emitir(estado: State<'_, Estado>, pedido: PedidoDto) -> R<ResultadoDto> {
    let pedido = pedido.pedido()?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    match emissor.emitir(&pedido).await.map_err(texto)? {
        ResultadoEmissao::Autorizada { nfse, avisos } => Ok(ResultadoDto {
            autorizada: true,
            nfse_id: Some(nfse.id),
            numero: nfse.numero,
            chave: Some(nfse.chave),
            avisos,
            motivo: None,
        }),
        ResultadoEmissao::Incerta { motivo, .. } => Ok(ResultadoDto {
            autorizada: false,
            nfse_id: None,
            numero: None,
            chave: None,
            avisos: vec![],
            motivo: Some(format!(
                "{motivo}. A nota pode ter sido autorizada: o sistema consulta a Sefin antes de qualquer reenvio."
            )),
        }),
    }
}

#[derive(Serialize)]
pub struct NotaDto {
    dps_id: i64,
    nfse_id: Option<i64>,
    serie: String,
    numero_dps: u64,
    numero_nfse: Option<String>,
    chave: Option<String>,
    situacao: &'static str,
    erro: Option<String>,
    tomador: Option<String>,
    descricao: Option<String>,
    valor: Option<String>,
    competencia: Option<String>,
    criada_em: String,
}

#[tauri::command]
pub async fn listar_notas(estado: State<'_, Estado>, empresa_id: i64, mes: Option<String>) -> R<Vec<NotaDto>> {
    let mes = vazio_para_none(mes);
    Ok(estado
        .banco
        .listar_notas(empresa_id, mes.as_deref())
        .await
        .map_err(texto)?
        .into_iter()
        .map(|n| NotaDto {
            dps_id: n.dps_id,
            nfse_id: n.nfse_id,
            serie: n.serie,
            numero_dps: n.numero_dps,
            numero_nfse: n.numero_nfse,
            chave: n.chave,
            situacao: n.situacao.texto(),
            erro: n.ultimo_erro,
            tomador: n.tomador,
            descricao: n.descricao,
            valor: n.valor.map(|v| v.to_string()),
            competencia: n.competencia,
            criada_em: n.criada_em,
        })
        .collect())
}

#[tauri::command]
pub async fn cancelar(
    estado: State<'_, Estado>,
    nfse_id: i64,
    motivo: MotivoCancelamento,
    justificativa: String,
) -> R<()> {
    let guarda = emissor!(estado);
    guarda
        .as_ref()
        .expect("verificado")
        .cancelar(nfse_id, motivo, &justificativa)
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn reconciliar(estado: State<'_, Estado>, dps_id: i64) -> R<String> {
    let guarda = emissor!(estado);
    Ok(
        match guarda
            .as_ref()
            .expect("verificado")
            .reconciliar(dps_id)
            .await
            .map_err(texto)?
        {
            Some(n) => format!(
                "A Sefin tinha autorizado: nota {} gravada.",
                n.numero.unwrap_or_default()
            ),
            None => "A Sefin não gerou nota para esta DPS; pode emitir de novo.".into(),
        },
    )
}

#[derive(Serialize)]
pub struct PendenteDto {
    dps_id: i64,
    empresa_id: i64,
    id_dps: String,
    erro: Option<String>,
}

#[tauri::command]
pub async fn pendentes(estado: State<'_, Estado>) -> R<Vec<PendenteDto>> {
    Ok(estado
        .banco
        .dps_pendentes()
        .await
        .map_err(texto)?
        .into_iter()
        .filter(|d| d.situacao == SituacaoDps::Incerta || d.situacao == SituacaoDps::Enviando)
        .map(|d| PendenteDto {
            dps_id: d.id,
            empresa_id: d.empresa_id,
            id_dps: d.id_dps,
            erro: d.ultimo_erro,
        })
        .collect())
}

/// Abre um arquivo ou pasta com o programa padrão do Windows.
#[tauri::command]
pub async fn abrir_arquivo(caminho: String) -> R<()> {
    std::process::Command::new("explorer")
        .arg(&caminho)
        .spawn()
        .map_err(texto)?;
    Ok(())
}

/// Abre o PDF (ou, com `pasta = true`, a pasta) da nota.
#[tauri::command]
pub async fn abrir_nota(estado: State<'_, Estado>, nfse_id: i64, pasta: bool) -> R<()> {
    let guarda = emissor!(estado);
    let xml = guarda
        .as_ref()
        .expect("verificado")
        .caminho_xml(nfse_id)
        .await
        .map_err(texto)?
        .ok_or("pasta de saída não configurada")?;
    let alvo = if pasta {
        xml.parent().map(PathBuf::from).unwrap_or(xml)
    } else {
        xml.with_extension("pdf")
    };
    if !alvo.exists() {
        return Err(format!("arquivo não encontrado: {}", alvo.display()));
    }
    std::process::Command::new("explorer")
        .arg(&alvo)
        .spawn()
        .map_err(texto)?;
    Ok(())
}

/// Consulta pública do CNPJ para sugerir nome, endereço e contato no cadastro.
#[tauri::command]
pub async fn consultar_cnpj(cnpj: String) -> R<crate::cnpj::DadosCnpj> {
    let cnpj = Cnpj::parse(&cnpj).map_err(|e| format!("CNPJ: {e}"))?;
    crate::cnpj::consultar(cnpj.as_str()).await
}
