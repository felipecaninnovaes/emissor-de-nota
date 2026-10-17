//! Comandos chamados pela interface. Erros voltam como texto pronto para mostrar ao operador.

use std::path::PathBuf;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use nfse_core::modelo::{Ambiente, RegimeTributario, Tomador};
use nfse_core::{Cnpj, Valor};
use nfse_db::dps::SituacaoDps;
use nfse_db::empresas::{Exclusao, NovaEmpresa};
use nfse_db::servicos::{Servico, Tributacao};
use nfse_db::tomadores::TomadorVinculado;
use nfse_emissor::{ConectorSefin, Emissor, Origem, PedidoEmissao, ResultadoEmissao};
use nfse_xml::MotivoCancelamento;

use crate::Estado;

pub(crate) type R<T> = Result<T, String>;

pub(crate) fn texto<E: std::fmt::Display>(e: E) -> String {
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

// ---------- aparência (lida antes da senha mestra; não tem dado sigiloso) ----------

const CHAVE_ZOOM: &str = "interface_zoom";
const CHAVE_DENSIDADE: &str = "interface_densidade";
const ZOOMS: [&str; 6] = ["0.9", "1", "1.1", "1.25", "1.4", "1.5"];
const DENSIDADES: [&str; 3] = ["auto", "confortavel", "compacta"];

#[derive(Serialize, Deserialize)]
pub struct Aparencia {
    /// Fator do tamanho do texto: "0.9", "1", "1.1", "1.25", "1.4" ou "1.5".
    zoom: String,
    /// "auto" (compacta em janelas baixas), "confortavel" ou "compacta".
    densidade: String,
}

#[tauri::command]
pub async fn obter_aparencia(estado: State<'_, Estado>) -> R<Aparencia> {
    let ler = |chave| estado.banco.configuracao(chave);
    let zoom = ler(CHAVE_ZOOM)
        .await
        .map_err(texto)?
        .filter(|z| ZOOMS.contains(&z.as_str()));
    let densidade = ler(CHAVE_DENSIDADE)
        .await
        .map_err(texto)?
        .filter(|d| DENSIDADES.contains(&d.as_str()));
    Ok(Aparencia {
        zoom: zoom.unwrap_or_else(|| "1".into()),
        densidade: densidade.unwrap_or_else(|| "auto".into()),
    })
}

#[tauri::command]
pub async fn salvar_aparencia(estado: State<'_, Estado>, aparencia: Aparencia) -> R<()> {
    if !ZOOMS.contains(&aparencia.zoom.as_str()) || !DENSIDADES.contains(&aparencia.densidade.as_str()) {
        return Err("opção de aparência inválida".into());
    }
    estado
        .banco
        .definir_configuracao(CHAVE_ZOOM, &aparencia.zoom)
        .await
        .map_err(texto)?;
    estado
        .banco
        .definir_configuracao(CHAVE_DENSIDADE, &aparencia.densidade)
        .await
        .map_err(texto)
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

pub(crate) fn vazio_para_none(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

impl NovaEmpresaDto {
    fn empresa(self) -> R<NovaEmpresa> {
        Ok(NovaEmpresa {
            cnpj: Cnpj::parse(&self.cnpj).map_err(|e| format!("CNPJ: {e}"))?,
            nome: self.nome,
            inscricao_municipal: vazio_para_none(self.inscricao_municipal),
            municipio_emissor: self.municipio_emissor.trim().to_owned(),
            regime: self.regime,
            ambiente: self.ambiente,
            serie: self.serie.trim().to_owned(),
            telefone: vazio_para_none(self.telefone).map(|t| t.chars().filter(char::is_ascii_digit).collect()),
            email: vazio_para_none(self.email),
        })
    }
}

#[tauri::command]
pub async fn cadastrar_empresa(estado: State<'_, Estado>, empresa: NovaEmpresaDto) -> R<i64> {
    let nova = empresa.empresa()?;
    Ok(estado.banco.cadastrar_empresa(&nova).await.map_err(texto)?.id)
}

#[tauri::command]
pub async fn atualizar_empresa(estado: State<'_, Estado>, empresa_id: i64, empresa: NovaEmpresaDto) -> R<()> {
    let dados = empresa.empresa()?;
    estado
        .banco
        .atualizar_empresa(empresa_id, &dados)
        .await
        .map_err(texto)?;
    Ok(())
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

/// Exclui a empresa (sem notas) ou arquiva (com notas); devolve a mensagem para o operador.
#[tauri::command]
pub async fn excluir_empresa(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    Ok(match estado.banco.excluir_empresa(empresa_id).await.map_err(texto)? {
        Exclusao::Excluida => "Empresa excluída.".into(),
        Exclusao::Arquivada { notas } => format!(
            "Empresa arquivada: ela tem {notas} NFS-e no banco, que continuam guardadas.              Cadastrar o mesmo CNPJ de novo traz a empresa de volta com as notas."
        ),
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

/// Pergunta à Sefin quais números da série atual já geraram NFS-e e ajusta o próximo.
#[tauri::command]
pub async fn ajustar_numeracao(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    let guarda = emissor!(estado);
    let (ultimo, proximo) = guarda
        .as_ref()
        .expect("verificado")
        .ajustar_numeracao_pela_sefin(empresa_id)
        .await
        .map_err(texto)?;
    Ok(match ultimo {
        Some(u) => format!("A Sefin já tem DPS desta série até o nº {u}. A próxima nota sai com o nº {proximo}."),
        None => format!("Nenhum número usado na Sefin a partir daqui. A próxima nota sai com o nº {proximo}."),
    })
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

impl From<ServicoDto> for Servico {
    fn from(servico: ServicoDto) -> Self {
        Servico {
            id: servico.id,
            empresa_id: servico.empresa_id,
            nome: servico.nome,
            descricao: servico.descricao,
            codigo_trib_nac: servico.codigo_trib_nac.chars().filter(char::is_ascii_digit).collect(),
            codigo_trib_mun: vazio_para_none(servico.codigo_trib_mun),
            nbs: vazio_para_none(servico.nbs).map(|n| n.chars().filter(char::is_ascii_digit).collect()),
            tributacao: servico.tributacao,
            valor_sugerido: servico.valor_sugerido,
        }
    }
}

#[tauri::command]
pub async fn salvar_servico(estado: State<'_, Estado>, servico: ServicoDto) -> R<i64> {
    estado.banco.salvar_servico(&servico.into()).await.map_err(texto)
}

#[derive(Serialize)]
pub struct LinhaServicoDto {
    linha: usize,
    servico: ServicoDto,
    erros: Vec<String>,
}

/// Lê a planilha de serviços para a tela conferir antes de importar.
#[tauri::command]
pub async fn ler_planilha_servicos(nome: String, bytes: Vec<u8>) -> R<Vec<LinhaServicoDto>> {
    Ok(nfse_emissor::planilha_servicos::ler(&nome, &bytes)?
        .into_iter()
        .map(|l| LinhaServicoDto {
            linha: l.numero,
            servico: l.servico.into(),
            erros: l.erros,
        })
        .collect())
}

#[derive(Serialize)]
pub struct ImportacaoServicosDto {
    empresa: String,
    novos: usize,
    atualizados: usize,
    /// Serviços cuja tributação a Sefin vai recusar para o regime desta empresa.
    avisos: Vec<String>,
}

/// Grava os serviços (da planilha ou copiados de outra empresa) em cada empresa escolhida.
/// Serviço com o mesmo nome já cadastrado é atualizado.
#[tauri::command]
pub async fn importar_servicos(
    estado: State<'_, Estado>,
    servicos: Vec<ServicoDto>,
    empresa_ids: Vec<i64>,
) -> R<Vec<ImportacaoServicosDto>> {
    if servicos.is_empty() {
        return Err("Nenhum serviço escolhido.".into());
    }
    if empresa_ids.is_empty() {
        return Err("Escolha ao menos uma empresa.".into());
    }
    let servicos: Vec<Servico> = servicos.into_iter().map(Into::into).collect();
    let b = &estado.banco;
    let mut resultado = Vec::with_capacity(empresa_ids.len());
    for id in empresa_ids {
        let empresa = b.empresa(id).await.map_err(texto)?;
        let resumo = b
            .importar_servicos(id, &servicos)
            .await
            .map_err(|e| format!("{}: {e}", empresa.nome))?;
        let avisos = servicos
            .iter()
            .filter_map(|s| {
                nfse_core::modelo::conferir_total_tributos(empresa.regime.simples, &s.tributacao.total_tributos)
                    .map(|motivo| format!("{}: {motivo}", s.nome.trim()))
            })
            .collect();
        resultado.push(ImportacaoServicosDto {
            empresa: empresa.nome,
            novos: resumo.novos,
            atualizados: resumo.atualizados,
            avisos,
        });
    }
    Ok(resultado)
}

#[tauri::command]
pub async fn salvar_modelo_servicos(estado: State<'_, Estado>) -> R<String> {
    gravar_e_abrir_csv(
        &estado,
        "modelo-importacao-servicos.csv",
        &nfse_emissor::planilha_servicos::modelo_csv(),
    )
    .await
}

/// Exporta os serviços da empresa no formato da importação (para levar a outras empresas).
#[tauri::command]
pub async fn exportar_servicos(estado: State<'_, Estado>, empresa_id: i64) -> R<String> {
    let b = &estado.banco;
    let empresa = b.empresa(empresa_id).await.map_err(texto)?;
    let servicos = b.servicos(empresa_id).await.map_err(texto)?;
    if servicos.is_empty() {
        return Err("Esta empresa não tem serviços para exportar.".into());
    }
    let nome = format!("servicos-{}.csv", empresa.cnpj.as_str());
    gravar_e_abrir_csv(&estado, &nome, &nfse_emissor::planilha_servicos::para_csv(&servicos)).await
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
    #[serde(default)]
    obra: Option<nfse_core::modelo::Obra>,
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
            obra: self.obra,
            informacoes_complementares: vazio_para_none(self.informacoes_complementares),
            origem: Origem::Manual,
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

/// Monta a DPS do pedido como ela sairia agora e lista os erros que impedem a emissão.
pub(crate) async fn validar_pedido(
    b: &nfse_db::Banco,
    pedido: &PedidoEmissao,
) -> R<(nfse_core::Dps, nfse_db::empresas::Empresa, Vec<String>)> {
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
        pedido,
        nfse_emissor::agora_brasilia(),
    );
    let mut erros = dps.validar();
    if b.certificado_resumo(empresa.id).await.map_err(texto)?.is_none() {
        erros.push("a empresa não tem certificado digital importado".into());
    }
    Ok((dps, empresa, erros))
}

/// Mostra como a DPS vai sair (descrição com o mês já aplicado) e os erros de validação, sem emitir.
#[tauri::command]
pub async fn previa_dps(estado: State<'_, Estado>, pedido: PedidoDto) -> R<PreviaDto> {
    let pedido = pedido.pedido()?;
    let (dps, empresa, erros) = validar_pedido(&estado.banco, &pedido).await?;
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

impl From<ResultadoEmissao> for ResultadoDto {
    fn from(r: ResultadoEmissao) -> Self {
        match r {
            ResultadoEmissao::Autorizada { nfse, avisos } => ResultadoDto {
                autorizada: true,
                nfse_id: Some(nfse.id),
                numero: nfse.numero,
                chave: Some(nfse.chave),
                avisos,
                motivo: None,
            },
            ResultadoEmissao::Incerta { motivo, .. } => ResultadoDto {
                autorizada: false,
                nfse_id: None,
                numero: None,
                chave: None,
                avisos: vec![],
                motivo: Some(format!(
                    "{motivo}. A nota pode ter sido autorizada: o sistema consulta a Sefin antes de qualquer reenvio."
                )),
            },
        }
    }
}

/// Converte o resultado e, se autorizada e o envio automático estiver ligado, manda o e-mail
/// (o resultado do envio entra nos avisos; falha no e-mail não desfaz a nota).
async fn concluir(emissor: &Emissor<ConectorSefin>, resultado: ResultadoEmissao) -> ResultadoDto {
    let nfse_id = match &resultado {
        ResultadoEmissao::Autorizada { nfse, .. } => Some(nfse.id),
        ResultadoEmissao::Incerta { .. } => None,
    };
    let mut dto = ResultadoDto::from(resultado);
    if let Some(id) = nfse_id
        && let Some(aviso) = emissor
            .enviar_email_apos_emissao(id, &nfse_emissor::email::CarteiroSmtp)
            .await
    {
        dto.avisos.push(aviso);
    }
    dto
}

#[tauri::command]
pub async fn emitir(estado: State<'_, Estado>, pedido: PedidoDto) -> R<ResultadoDto> {
    let pedido = pedido.pedido()?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor.emitir(&pedido).await.map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
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
    email_enviado_em: Option<String>,
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
            email_enviado_em: n.email_enviado_em,
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
pub struct RefazerDto {
    empresa_id: i64,
    tomador: Option<TomadorDto>,
    servico_id: Option<i64>,
    valor: String,
    descricao: String,
    obra: Option<nfse_core::modelo::Obra>,
    informacoes_complementares: Option<String>,
}

/// Dados de uma nota rejeitada para preencher a tela de emissão de novo.
#[tauri::command]
pub async fn dados_para_refazer(estado: State<'_, Estado>, dps_id: i64) -> R<RefazerDto> {
    let guarda = emissor!(estado);
    let r = guarda
        .as_ref()
        .expect("verificado")
        .dados_para_refazer(dps_id)
        .await
        .map_err(texto)?;
    Ok(RefazerDto {
        empresa_id: r.empresa_id,
        tomador: r.tomador.map(Into::into),
        servico_id: r.servico_id,
        valor: r.valor.to_string(),
        descricao: r.descricao,
        obra: r.obra,
        informacoes_complementares: r.informacoes_complementares,
    })
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

/// Consulta pública do CEP para sugerir logradouro, bairro e município.
#[tauri::command]
pub async fn consultar_cep(cep: String) -> R<crate::cep::DadosCep> {
    let cep: String = cep.chars().filter(char::is_ascii_digit).collect();
    if cep.len() != 8 {
        return Err("CEP com 8 dígitos".into());
    }
    crate::cep::consultar(&cep).await
}

// ---------- notas recorrentes e lote do mês ----------

fn mes_de(mes: &str) -> R<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{}-01", mes.trim()), "%Y-%m-%d")
        .map_err(|_| "mês de referência inválido".into())
}

#[derive(Serialize)]
pub struct TentativaDto {
    dps_id: i64,
    situacao: &'static str,
    numero_nfse: Option<String>,
    nfse_id: Option<i64>,
    erro: Option<String>,
    /// Rejeitada ou cancelada: pode emitir de novo neste mês.
    libera: bool,
}

#[derive(Serialize)]
pub struct RecorrenciaDto {
    id: i64,
    empresa_id: i64,
    empresa: String,
    tomador_id: Option<i64>,
    tomador: Option<String>,
    servico_id: i64,
    servico: String,
    /// Valor próprio da recorrência (`None` = usa o do serviço).
    valor: Option<String>,
    valor_efetivo: Option<String>,
    descricao: Option<String>,
    /// Descrição que vai na nota, com o mês aplicado.
    descricao_no_mes: String,
    informacoes_complementares: Option<String>,
    no_mes: Option<TentativaDto>,
    /// O que impede a emissão no mês (vazio = pode emitir).
    erros: Vec<String>,
}

/// Recorrências (da empresa ou de todas, com `empresa_id` nulo) e a situação de cada uma no mês.
#[tauri::command]
pub async fn listar_recorrencias(
    estado: State<'_, Estado>,
    empresa_id: Option<i64>,
    mes: String,
) -> R<Vec<RecorrenciaDto>> {
    let data = mes_de(&mes)?;
    let b = &estado.banco;
    let mut lista = Vec::new();
    for item in b.recorrencias(empresa_id, mes.trim()).await.map_err(texto)? {
        let r = &item.recorrencia;
        let mut erros = Vec::new();
        if !item.servico_ativo {
            erros.push(format!(
                "o serviço \"{}\" foi removido; edite a recorrência",
                item.servico
            ));
        }
        match item.valor_efetivo {
            Some(valor) if item.servico_ativo => {
                let pedido = PedidoEmissao {
                    empresa_id: r.empresa_id,
                    tomador: r.tomador,
                    servico_id: r.servico_id,
                    valor,
                    descricao: r.descricao.clone(),
                    mes_referencia: Some(data),
                    obra: None,
                    informacoes_complementares: r.informacoes_complementares.clone(),
                    origem: r.id.map(Origem::Recorrencia).unwrap_or_default(),
                };
                match validar_pedido(b, &pedido).await {
                    Ok((_, _, e)) => erros.extend(e),
                    Err(e) => erros.push(e),
                }
            }
            Some(_) => {}
            None => erros.push("sem valor".into()),
        }
        lista.push(RecorrenciaDto {
            id: r.id.expect("gravada"),
            empresa_id: r.empresa_id,
            empresa: item.empresa.clone(),
            tomador_id: r.tomador,
            tomador: item.tomador.clone(),
            servico_id: r.servico_id,
            servico: item.servico.clone(),
            valor: r.valor.map(|v| v.to_string()),
            valor_efetivo: item.valor_efetivo.map(|v| v.to_string()),
            descricao: r.descricao.clone(),
            descricao_no_mes: nfse_emissor::aplicar_mes_ref(&item.descricao_efetiva, data),
            informacoes_complementares: r.informacoes_complementares.clone(),
            no_mes: item.no_mes.map(|t| TentativaDto {
                libera: t.libera_o_mes(),
                dps_id: t.dps_id,
                situacao: t.situacao.texto(),
                numero_nfse: t.numero_nfse,
                nfse_id: t.nfse_id,
                erro: t.erro,
            }),
            erros,
        });
    }
    Ok(lista)
}

#[derive(Deserialize)]
pub struct RecorrenciaEntrada {
    id: Option<i64>,
    empresa_id: i64,
    tomador: Option<i64>,
    servico_id: i64,
    valor: Option<Valor>,
    descricao: Option<String>,
    informacoes_complementares: Option<String>,
}

#[tauri::command]
pub async fn salvar_recorrencia(estado: State<'_, Estado>, recorrencia: RecorrenciaEntrada) -> R<i64> {
    estado
        .banco
        .salvar_recorrencia(&nfse_db::recorrencias::Recorrencia {
            id: recorrencia.id,
            empresa_id: recorrencia.empresa_id,
            tomador: recorrencia.tomador,
            servico_id: recorrencia.servico_id,
            valor: recorrencia.valor,
            descricao: vazio_para_none(recorrencia.descricao),
            informacoes_complementares: vazio_para_none(recorrencia.informacoes_complementares),
        })
        .await
        .map_err(texto)
}

#[tauri::command]
pub async fn desativar_recorrencia(estado: State<'_, Estado>, id: i64) -> R<()> {
    estado.banco.desativar_recorrencia(id).await.map_err(texto)
}

/// Emite uma recorrência no mês (`AAAA-MM`); o lote chama um item por vez para mostrar o andamento.
#[tauri::command]
pub async fn emitir_recorrencia(estado: State<'_, Estado>, recorrencia_id: i64, mes: String) -> R<ResultadoDto> {
    let data = mes_de(&mes)?;
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor.emitir_recorrencia(recorrencia_id, data).await.map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}

// ---------- importação de planilha (convênios) ----------

#[derive(Serialize, Deserialize)]
pub struct ItemPlanilhaDto {
    linha: usize,
    empresa_id: Option<i64>,
    empresa: Option<String>,
    pessoa_id: Option<i64>,
    documento_tomador: String,
    tomador: Option<String>,
    vincular_tomador: bool,
    servico_id: Option<i64>,
    servico: Option<String>,
    valor: Option<Valor>,
    /// `AAAA-MM`
    mes: String,
    descricao: String,
    informacoes: Option<String>,
    chave: String,
    #[serde(default, skip_deserializing)]
    anterior: Option<TentativaDto>,
    #[serde(default)]
    erros: Vec<String>,
}

fn tentativa_dto(t: nfse_db::recorrencias::TentativaNoMes) -> TentativaDto {
    TentativaDto {
        libera: t.libera_o_mes(),
        dps_id: t.dps_id,
        situacao: t.situacao.texto(),
        numero_nfse: t.numero_nfse,
        nfse_id: t.nfse_id,
        erro: t.erro,
    }
}

/// Lê a planilha e devolve cada linha resolvida contra o cadastro, com o que impede a emissão.
#[tauri::command]
pub async fn ler_planilha(
    estado: State<'_, Estado>,
    nome: String,
    bytes: Vec<u8>,
    empresa_id: Option<i64>,
    mes: String,
) -> R<Vec<ItemPlanilhaDto>> {
    let mes_padrao = mes_de(&mes)?;
    let linhas = nfse_emissor::planilha::ler(&nome, &bytes)?;
    let b = &estado.banco;
    let itens = nfse_emissor::importacao::resolver(b, &linhas, empresa_id, mes_padrao)
        .await
        .map_err(texto)?;
    let mut lista = Vec::with_capacity(itens.len());
    for item in itens {
        let mut erros = item.erros.clone();
        if erros.is_empty()
            && let Some(mut pedido) = item.pedido(None)
        {
            // Tomador ainda não vinculado à empresa: confere o resto (ele é vinculado ao emitir).
            if item.vincular_tomador {
                pedido.tomador = None;
            }
            match validar_pedido(b, &pedido).await {
                Ok((_, _, e)) => erros.extend(e),
                Err(e) => erros.push(e),
            }
        }
        lista.push(ItemPlanilhaDto {
            linha: item.linha,
            empresa_id: item.empresa_id,
            empresa: item.empresa,
            pessoa_id: item.pessoa_id,
            documento_tomador: item.documento_tomador,
            tomador: item.tomador,
            vincular_tomador: item.vincular_tomador,
            servico_id: item.servico_id,
            servico: item.servico,
            valor: item.valor,
            mes: item.mes.format("%Y-%m").to_string(),
            descricao: item.descricao,
            informacoes: item.informacoes,
            chave: item.chave,
            anterior: item.anterior.map(tentativa_dto),
            erros,
        });
    }
    Ok(lista)
}

/// Emite uma linha da planilha (a tela chama uma por vez para mostrar o andamento).
#[tauri::command]
pub async fn emitir_planilha(
    estado: State<'_, Estado>,
    item: ItemPlanilhaDto,
    arquivo: Option<String>,
) -> R<ResultadoDto> {
    let importado = nfse_emissor::importacao::ItemImportado {
        linha: item.linha,
        empresa_id: item.empresa_id,
        empresa: item.empresa,
        pessoa_id: item.pessoa_id,
        documento_tomador: item.documento_tomador,
        tomador: item.tomador,
        vincular_tomador: item.vincular_tomador,
        servico_id: item.servico_id,
        servico: item.servico,
        valor: item.valor,
        mes: mes_de(&item.mes)?,
        descricao: item.descricao,
        informacoes: item.informacoes,
        chave: item.chave,
        anterior: None,
        erros: vec![],
    };
    let guarda = emissor!(estado);
    let emissor = guarda.as_ref().expect("verificado");
    let resultado = emissor
        .emitir_importado(&importado, arquivo.as_deref())
        .await
        .map_err(texto)?;
    Ok(concluir(emissor, resultado).await)
}

/// Grava o modelo de planilha na pasta das notas e abre no programa padrão (Excel).
#[tauri::command]
pub async fn salvar_modelo_planilha(estado: State<'_, Estado>) -> R<String> {
    gravar_e_abrir_csv(
        &estado,
        "modelo-importacao-notas.csv",
        &nfse_emissor::planilha::modelo_csv(),
    )
    .await
}

/// Grava o CSV na pasta das notas e abre no programa padrão (Excel). Devolve o caminho.
async fn gravar_e_abrir_csv(estado: &Estado, nome: &str, conteudo: &str) -> R<String> {
    let pasta = pasta_configurada(estado).await?;
    std::fs::create_dir_all(&pasta).map_err(texto)?;
    let caminho = pasta.join(nome);
    // BOM: o Excel abre o CSV em UTF-8 com os acentos certos.
    std::fs::write(&caminho, format!("\u{feff}{conteudo}"))
        .map_err(|e| format!("não foi possível gravar {nome}: {e}"))?;
    std::process::Command::new("explorer")
        .arg(&caminho)
        .spawn()
        .map_err(texto)?;
    Ok(caminho.display().to_string())
}

// ---------- e-mail ----------

#[derive(Serialize)]
pub struct ConfigEmailDto {
    config: nfse_db::emails::ConfigSmtp,
    /// Já existe senha guardada (o campo da tela fica vazio para não mostrá-la).
    tem_senha: bool,
    configurado: bool,
}

#[tauri::command]
pub async fn obter_email(estado: State<'_, Estado>) -> R<ConfigEmailDto> {
    let b = &estado.banco;
    let gravada = b.smtp().await.map_err(texto)?;
    Ok(ConfigEmailDto {
        configurado: gravada.is_some(),
        config: gravada.unwrap_or_else(|| nfse_db::emails::ConfigSmtp {
            servidor: String::new(),
            porta: 587,
            seguranca: nfse_db::emails::Seguranca::StartTls,
            usuario: None,
            remetente: String::new(),
            nome_remetente: None,
            assunto: nfse_db::emails::ConfigSmtp::ASSUNTO_PADRAO.into(),
            corpo: nfse_db::emails::ConfigSmtp::CORPO_PADRAO.into(),
            enviar_ao_emitir: false,
        }),
        tem_senha: b.tem_senha_smtp().await.map_err(texto)?,
    })
}

/// Grava a configuração; `senha` vazia mantém a que já estava guardada.
#[tauri::command]
pub async fn salvar_email(
    estado: State<'_, Estado>,
    config: nfse_db::emails::ConfigSmtp,
    senha: Option<String>,
) -> R<()> {
    let guarda = emissor!(estado);
    let cofre = guarda.as_ref().expect("verificado").cofre();
    estado.banco.definir_smtp(&config).await.map_err(texto)?;
    if let Some(senha) = senha.filter(|s| !s.is_empty()) {
        estado.banco.definir_senha_smtp(cofre, &senha).await.map_err(texto)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn testar_email(estado: State<'_, Estado>, para: String) -> R<()> {
    let guarda = emissor!(estado);
    guarda
        .as_ref()
        .expect("verificado")
        .enviar_email_teste(para.trim(), &nfse_emissor::email::CarteiroSmtp)
        .await
        .map_err(texto)
}

/// Destinatários padrão da nota (e-mails de envio do tomador ou o e-mail do cadastro).
#[tauri::command]
pub async fn destinatarios_email(estado: State<'_, Estado>, nfse_id: i64) -> R<Vec<String>> {
    Ok(estado
        .banco
        .dados_email_nfse(nfse_id)
        .await
        .map_err(texto)?
        .destinatarios)
}

#[tauri::command]
pub async fn enviar_email(estado: State<'_, Estado>, nfse_id: i64, para: String) -> R<String> {
    let para = nfse_db::emails::separar_emails(&para);
    let guarda = emissor!(estado);
    let enviados = guarda
        .as_ref()
        .expect("verificado")
        .enviar_email(nfse_id, &para, &nfse_emissor::email::CarteiroSmtp)
        .await
        .map_err(texto)?;
    Ok(format!("E-mail enviado para {}.", enviados.join(", ")))
}

// ---------- atualização do aplicativo ----------

#[derive(Serialize)]
pub struct AtualizacaoDto {
    versao: String,
    notas: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ProgressoAtualizacao {
    baixado: usize,
    total: Option<u64>,
}

/// Versão instalada (a de `crates/app/Cargo.toml`).
#[tauri::command]
pub fn versao_app(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Consulta a última Release no GitHub; `None` quando já está na versão mais nova.
#[tauri::command]
pub async fn procurar_atualizacao(app: tauri::AppHandle) -> R<Option<AtualizacaoDto>> {
    use tauri_plugin_updater::UpdaterExt;
    let nova = app.updater().map_err(texto)?.check().await.map_err(texto)?;
    Ok(nova.map(|u| AtualizacaoDto {
        versao: u.version,
        notas: u.body,
    }))
}

/// Baixa, confere a assinatura e instala a nova versão. No Windows o instalador fecha o sistema e
/// o abre de novo ao terminar. O andamento sai no evento `progresso-atualizacao`.
#[tauri::command]
pub async fn instalar_atualizacao(app: tauri::AppHandle) -> R<()> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;
    let Some(nova) = app.updater().map_err(texto)?.check().await.map_err(texto)? else {
        return Err("o sistema já está na versão mais nova".into());
    };
    let mut baixado = 0;
    nova.download_and_install(
        |pedaco, total| {
            baixado += pedaco;
            let _ = app.emit("progresso-atualizacao", ProgressoAtualizacao { baixado, total });
        },
        || {},
    )
    .await
    .map_err(|e| format!("não foi possível atualizar: {e}"))
}
