//! Modelo da DPS (leiaute 1.01) com os grupos que o escritório usa: prestador emitente,
//! tomador nacional, um serviço, ISSQN (com retenção), retenções federais, total de
//! tributos e o grupo IBS/CBS.
//!
//! Grupos ainda não modelados (comércio exterior, evento, deduções, intermediário,
//! reembolso IBS/CBS) entram quando houver cliente que precise deles.
//!
//! Os nomes dos campos seguem o leiaute em português legível; a tag XML de cada um está
//! no comentário. A ordem dos elementos é responsabilidade do gerador em `nfse-xml`.

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::dps::{DpsId, DpsIdInvalido, Inscricao};
use crate::{Cnpj, Cpf, Percentual, Valor};

/// Versão do leiaute gerado (`DPS/@versao`).
pub const VERSAO_LEIAUTE: &str = "1.01";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ambiente {
    Producao,
    /// Produção Restrita (homologação).
    Homologacao,
}

impl Ambiente {
    /// `tpAmb`
    pub fn codigo(self) -> &'static str {
        match self {
            Ambiente::Producao => "1",
            Ambiente::Homologacao => "2",
        }
    }
}

/// CNPJ ou CPF (grupo de escolha `CNPJ|CPF` do leiaute).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Documento {
    Cnpj(Cnpj),
    Cpf(Cpf),
}

impl Documento {
    pub fn inscricao(&self) -> Inscricao {
        match self {
            Documento::Cnpj(c) => Inscricao::Cnpj(c.clone()),
            Documento::Cpf(c) => Inscricao::Cpf(c.clone()),
        }
    }
}

/// `opSimpNac`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoSimples {
    NaoOptante,
    Mei,
    MeEpp,
}

impl SituacaoSimples {
    pub fn codigo(self) -> &'static str {
        match self {
            SituacaoSimples::NaoOptante => "1",
            SituacaoSimples::Mei => "2",
            SituacaoSimples::MeEpp => "3",
        }
    }
}

/// `regApTribSN` (só para ME/EPP optante).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApuracaoSimples {
    /// 1 – tributos federais e municipal pelo SN.
    TudoNoSimples,
    /// 2 – federais pelo SN e ISSQN por fora.
    IssqnPorFora,
    /// 3 – federais e municipal por fora do SN.
    TudoPorFora,
}

impl ApuracaoSimples {
    pub fn codigo(self) -> &'static str {
        match self {
            ApuracaoSimples::TudoNoSimples => "1",
            ApuracaoSimples::IssqnPorFora => "2",
            ApuracaoSimples::TudoPorFora => "3",
        }
    }
}

/// `regTrib`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegimeTributario {
    pub simples: SituacaoSimples,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apuracao_simples: Option<ApuracaoSimples>,
    /// `regEspTrib`: 0 nenhum, 1 cooperativa, 2 estimativa, 3 ME municipal, 4 notário,
    /// 5 autônomo, 6 sociedade de profissionais, 9 outros.
    #[serde(default)]
    pub regime_especial: u8,
}

/// `prest` quando o prestador é o emitente (`tpEmit` = 1). Nesse caso o leiaute não aceita
/// nome e endereço do prestador: a Sefin usa o cadastro nacional (CNC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prestador {
    pub documento: Documento,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inscricao_municipal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telefone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub regime: RegimeTributario,
}

/// `end` com `endNac`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnderecoNacional {
    /// `cMun` (IBGE, 7 dígitos)
    pub municipio: String,
    pub cep: String,
    pub logradouro: String,
    pub numero: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complemento: Option<String>,
    pub bairro: String,
}

/// `toma`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tomador {
    pub documento: Documento,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inscricao_municipal: Option<String>,
    pub nome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endereco: Option<EnderecoNacional>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telefone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// `serv`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Servico {
    /// `locPrest/cLocPrestacao` (IBGE)
    pub municipio_prestacao: String,
    /// `cTribNac`, 6 dígitos sem pontos (item, subitem e desdobramento da LC 116).
    pub codigo_tributacao_nacional: String,
    /// `cTribMun`, 3 dígitos, quando o município exigir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codigo_tributacao_municipal: Option<String>,
    /// `xDescServ`
    pub descricao: String,
    /// `cNBS`, 9 dígitos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbs: Option<String>,
    /// `obra`: obrigatório para os códigos de [`exige_obra`] (regra E0370).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obra: Option<Obra>,
    /// `infoCompl/xInfComp`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub informacoes_complementares: Option<String>,
}

/// Códigos de tributação nacional (subitens 07.02.01 a 14.14.04) que exigem o grupo de obra (E0370).
pub const CODIGOS_COM_OBRA: [&str; 13] = [
    "070201", "070202", "070401", "070501", "070502", "070601", "070602", "070701", "070801", "071701", "071901",
    "141403", "141404",
];

pub fn exige_obra(codigo_tributacao_nacional: &str) -> bool {
    CODIGOS_COM_OBRA.contains(&codigo_tributacao_nacional)
}

/// `obra`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obra {
    /// `inscImobFisc`: inscrição imobiliária fiscal (prefeitura).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inscricao_imobiliaria: Option<String>,
    pub identificacao: IdentificacaoObra,
}

/// Escolha `cObra | cCIB | end` do grupo de obra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificacaoObra {
    /// `cObra`: Cadastro Nacional de Obras (CNO) ou CEI.
    Cno(String),
    /// `cCIB`: Cadastro Imobiliário Brasileiro, 8 caracteres.
    Cib(String),
    /// `end`: endereço da obra (nacional, com CEP).
    Endereco(EnderecoObra),
}

/// `end` do grupo de obra (`TCEnderObraEvento`, só a forma nacional).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnderecoObra {
    pub cep: String,
    pub logradouro: String,
    pub numero: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complemento: Option<String>,
    pub bairro: String,
}

/// `tribISSQN`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TributacaoIssqn {
    #[default]
    Tributavel,
    Imunidade,
    Exportacao,
    NaoIncidencia,
}

impl TributacaoIssqn {
    pub fn codigo(self) -> &'static str {
        match self {
            TributacaoIssqn::Tributavel => "1",
            TributacaoIssqn::Imunidade => "2",
            TributacaoIssqn::Exportacao => "3",
            TributacaoIssqn::NaoIncidencia => "4",
        }
    }
}

/// `tpRetISSQN`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetencaoIssqn {
    #[default]
    NaoRetido,
    RetidoPeloTomador,
    RetidoPeloIntermediario,
}

impl RetencaoIssqn {
    pub fn codigo(self) -> &'static str {
        match self {
            RetencaoIssqn::NaoRetido => "1",
            RetencaoIssqn::RetidoPeloTomador => "2",
            RetencaoIssqn::RetidoPeloIntermediario => "3",
        }
    }
}

/// `tribMun`
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Issqn {
    #[serde(default)]
    pub tributacao: TributacaoIssqn,
    #[serde(default)]
    pub retencao: RetencaoIssqn,
    /// `pAliq`: só quando a regra do município/regime pedir (ex.: Simples com ISS retido).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota: Option<Percentual>,
}

/// `tribFed/piscofins`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PisCofins {
    /// `CST` (2 dígitos; "00" nenhum, "01" tributável alíquota básica...).
    pub cst: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calculo: Option<Valor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota_pis: Option<Percentual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota_cofins: Option<Percentual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor_pis: Option<Valor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor_cofins: Option<Valor>,
    /// `tpRetPisCofins` (0 a 9; ver leiaute).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipo_retencao: Option<u8>,
}

/// `tribFed`
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TributosFederais {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pis_cofins: Option<PisCofins>,
    /// `vRetCP` (contribuição previdenciária retida, INSS)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retencao_cp: Option<Valor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retencao_irrf: Option<Valor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retencao_csll: Option<Valor>,
}

impl TributosFederais {
    pub fn vazio(&self) -> bool {
        self == &Self::default()
    }
}

/// `totTrib` (Lei da Transparência, 12.741/2012).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TotalTributos {
    /// `indTotTrib` = 0: não informar estimativa.
    #[default]
    NaoInformado,
    /// `pTotTribSN`: percentual da alíquota do Simples Nacional.
    PercentualSimples(Percentual),
    /// `pTotTrib`
    Percentuais {
        federal: Percentual,
        estadual: Percentual,
        municipal: Percentual,
    },
    /// `vTotTrib`
    Valores {
        federal: Valor,
        estadual: Valor,
        municipal: Valor,
    },
}

/// Anexo I, regras E0710/E0712/E0713: a forma do total de tributos depende do Simples.
/// Devolve o motivo da rejeição, se a combinação não for aceita.
pub fn conferir_total_tributos(simples: SituacaoSimples, total: &TotalTributos) -> Option<&'static str> {
    match (simples, total) {
        (SituacaoSimples::MeEpp, TotalTributos::NaoInformado) => {
            Some("ME/EPP do Simples deve informar o % do Simples Nacional (E0712); ajuste o serviço")
        }
        (SituacaoSimples::NaoOptante, TotalTributos::NaoInformado | TotalTributos::PercentualSimples(_)) => Some(
            "não optante do Simples deve informar os percentuais federal, estadual e municipal (E0713); ajuste o serviço",
        ),
        (SituacaoSimples::Mei, TotalTributos::PercentualSimples(_)) => {
            Some("MEI não pode informar o % do Simples Nacional (E0710); ajuste o serviço")
        }
        _ => None,
    }
}

/// `valores`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Valores {
    /// `vServPrest/vServ`
    pub servico: Valor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desconto_incondicionado: Option<Valor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desconto_condicionado: Option<Valor>,
    #[serde(default)]
    pub issqn: Issqn,
    #[serde(default, skip_serializing_if = "TributosFederais::vazio")]
    pub federais: TributosFederais,
    #[serde(default)]
    pub total_tributos: TotalTributos,
}

/// Grupo `IBSCBS` (Reforma Tributária). Em 2026 é informativo; os códigos vêm das tabelas
/// oficiais (cIndOp: anexo de indicador de operação; CST/cClassTrib: tabela do IBS/CBS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IbsCbs {
    /// `indFinal`: operação de uso ou consumo pessoal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumo_pessoal: Option<bool>,
    /// `cIndOp`, 6 dígitos.
    pub indicador_operacao: String,
    /// `indDest`: `false` = o destinatário é o próprio tomador (0).
    #[serde(default)]
    pub destinatario_diferente_do_tomador: bool,
    /// `CST`, 3 dígitos.
    pub cst: String,
    /// `cClassTrib`, 6 dígitos.
    pub classificacao_tributaria: String,
}

/// Dados da NFS-e substituída (`subst`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Substituicao {
    pub chave_substituida: String,
    /// `cMotivo`: 01 a 05 ou 99.
    pub motivo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descricao_motivo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dps {
    pub ambiente: Ambiente,
    /// `dhEmi`, com fuso (Brasília: -03:00). Não pode estar no futuro em relação à Sefin.
    pub emissao: DateTime<FixedOffset>,
    /// `verAplic`
    pub versao_aplicativo: String,
    pub serie: String,
    pub numero: u64,
    /// `dCompet`
    pub competencia: NaiveDate,
    /// `cLocEmi`: município do emitente (IBGE).
    pub municipio_emissor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substituicao: Option<Substituicao>,
    pub prestador: Prestador,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tomador: Option<Tomador>,
    pub servico: Servico,
    pub valores: Valores,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ibs_cbs: Option<IbsCbs>,
}

impl Dps {
    pub fn id(&self) -> Result<DpsId, DpsIdInvalido> {
        DpsId::novo(
            &self.municipio_emissor,
            &self.prestador.documento.inscricao(),
            &self.serie,
            self.numero,
        )
    }

    /// Confere tamanhos, padrões e faixas do XSD antes de gerar o XML.
    /// Devolve todas as violações encontradas, com o caminho do campo.
    pub fn validar(&self) -> Vec<String> {
        let mut v = Validador::default();
        if let Err(e) = self.id() {
            v.erro("id", e.to_string());
        }
        v.texto("versao_aplicativo", &self.versao_aplicativo, 1, 20);
        v.serie("serie", &self.serie);
        v.digitos("municipio_emissor", &self.municipio_emissor, 7, 7);

        if let Some(s) = &self.substituicao {
            v.digitos("substituicao.chave_substituida", &s.chave_substituida, 50, 50);
            if !["01", "02", "03", "04", "05", "99"].contains(&s.motivo.as_str()) {
                v.erro("substituicao.motivo", "deve ser 01, 02, 03, 04, 05 ou 99");
            }
            if let Some(d) = &s.descricao_motivo {
                v.texto("substituicao.descricao_motivo", d, 15, 255);
            }
        }

        let p = &self.prestador;
        v.opcional_texto("prestador.inscricao_municipal", &p.inscricao_municipal, 1, 15);
        v.opcional_telefone("prestador.telefone", &p.telefone);
        v.opcional_texto("prestador.email", &p.email, 1, 80);
        if p.regime.apuracao_simples.is_some() && p.regime.simples != SituacaoSimples::MeEpp {
            v.erro(
                "prestador.regime.apuracao_simples",
                "só se aplica a ME/EPP optante do Simples",
            );
        }
        if ![0, 1, 2, 3, 4, 5, 6, 9].contains(&p.regime.regime_especial) {
            v.erro("prestador.regime.regime_especial", "deve ser 0 a 6 ou 9");
        }

        if let Some(t) = &self.tomador {
            v.opcional_texto("tomador.inscricao_municipal", &t.inscricao_municipal, 1, 15);
            v.texto("tomador.nome", &t.nome, 1, 300);
            v.opcional_telefone("tomador.telefone", &t.telefone);
            v.opcional_texto("tomador.email", &t.email, 1, 80);
            if let Some(e) = &t.endereco {
                v.digitos("tomador.endereco.municipio", &e.municipio, 7, 7);
                v.digitos("tomador.endereco.cep", &e.cep, 8, 8);
                v.texto("tomador.endereco.logradouro", &e.logradouro, 1, 255);
                v.texto("tomador.endereco.numero", &e.numero, 1, 60);
                v.opcional_texto("tomador.endereco.complemento", &e.complemento, 1, 156);
                v.texto("tomador.endereco.bairro", &e.bairro, 1, 60);
            }
        }

        let s = &self.servico;
        v.digitos("servico.municipio_prestacao", &s.municipio_prestacao, 7, 7);
        v.digitos(
            "servico.codigo_tributacao_nacional",
            &s.codigo_tributacao_nacional,
            6,
            6,
        );
        if let Some(c) = &s.codigo_tributacao_municipal {
            v.digitos("servico.codigo_tributacao_municipal", c, 3, 3);
        }
        v.texto("servico.descricao", &s.descricao, 1, 2000);
        if let Some(n) = &s.nbs {
            v.digitos("servico.nbs", n, 9, 9);
        }
        match &s.obra {
            None if exige_obra(&s.codigo_tributacao_nacional) => v.erro(
                "servico.obra",
                "este código de serviço exige os dados da obra (E0370): informe o CNO, o CIB ou o endereço da obra",
            ),
            None => {}
            Some(o) => {
                v.opcional_texto("servico.obra.inscricao_imobiliaria", &o.inscricao_imobiliaria, 1, 30);
                match &o.identificacao {
                    IdentificacaoObra::Cno(c) => v.texto("servico.obra.cno", c, 1, 30),
                    IdentificacaoObra::Cib(c) => {
                        if c.chars().count() != 8 {
                            v.erro("servico.obra.cib", "deve ter 8 caracteres");
                        }
                    }
                    IdentificacaoObra::Endereco(e) => {
                        v.digitos("servico.obra.endereco.cep", &e.cep, 8, 8);
                        v.texto("servico.obra.endereco.logradouro", &e.logradouro, 1, 255);
                        v.texto("servico.obra.endereco.numero", &e.numero, 1, 60);
                        v.opcional_texto("servico.obra.endereco.complemento", &e.complemento, 1, 156);
                        v.texto("servico.obra.endereco.bairro", &e.bairro, 1, 60);
                    }
                }
            }
        }
        v.opcional_texto(
            "servico.informacoes_complementares",
            &s.informacoes_complementares,
            1,
            2000,
        );

        let val = &self.valores;
        v.valor15("valores.servico", val.servico);
        v.opcional_valor15("valores.desconto_incondicionado", val.desconto_incondicionado);
        v.opcional_valor15("valores.desconto_condicionado", val.desconto_condicionado);
        if let Some(erro) = conferir_total_tributos(p.regime.simples, &val.total_tributos) {
            v.erro("valores.total_tributos", erro);
        }
        if let Some(a) = val.issqn.aliquota {
            v.percentual("valores.issqn.aliquota", a, 1);
        }
        if let Some(pc) = &val.federais.pis_cofins {
            v.digitos("valores.federais.pis_cofins.cst", &pc.cst, 2, 2);
            v.opcional_valor15("valores.federais.pis_cofins.base_calculo", pc.base_calculo);
            for (campo, p) in [
                ("aliquota_pis", pc.aliquota_pis),
                ("aliquota_cofins", pc.aliquota_cofins),
            ] {
                if let Some(p) = p {
                    v.percentual(&format!("valores.federais.pis_cofins.{campo}"), p, 2);
                }
            }
            v.opcional_valor15("valores.federais.pis_cofins.valor_pis", pc.valor_pis);
            v.opcional_valor15("valores.federais.pis_cofins.valor_cofins", pc.valor_cofins);
            if pc.tipo_retencao.is_some_and(|t| t > 9) {
                v.erro("valores.federais.pis_cofins.tipo_retencao", "deve ser 0 a 9");
            }
        }
        v.opcional_valor15("valores.federais.retencao_cp", val.federais.retencao_cp);
        v.opcional_valor15("valores.federais.retencao_irrf", val.federais.retencao_irrf);
        v.opcional_valor15("valores.federais.retencao_csll", val.federais.retencao_csll);
        match &val.total_tributos {
            TotalTributos::NaoInformado => {}
            TotalTributos::PercentualSimples(p) => v.percentual("valores.total_tributos", *p, 2),
            TotalTributos::Percentuais {
                federal,
                estadual,
                municipal,
            } => {
                for p in [federal, estadual, municipal] {
                    v.percentual("valores.total_tributos", *p, 3);
                }
            }
            TotalTributos::Valores {
                federal,
                estadual,
                municipal,
            } => {
                for x in [federal, estadual, municipal] {
                    v.valor15("valores.total_tributos", *x);
                }
            }
        }

        // Reforma Tributária: grupo IBSCBS obrigatório para o regime regular desde 08/2026.
        if self.ibs_cbs.is_none() && p.regime.simples == SituacaoSimples::NaoOptante {
            v.erro(
                "ibs_cbs",
                "empresa fora do Simples Nacional deve informar IBS/CBS; preencha o grupo no cadastro do serviço",
            );
        }
        if let Some(i) = &self.ibs_cbs {
            v.digitos("ibs_cbs.indicador_operacao", &i.indicador_operacao, 6, 6);
            v.digitos("ibs_cbs.cst", &i.cst, 3, 3);
            v.digitos("ibs_cbs.classificacao_tributaria", &i.classificacao_tributaria, 6, 6);
        }
        v.erros
    }
}

#[derive(Default)]
struct Validador {
    erros: Vec<String>,
}

impl Validador {
    fn erro(&mut self, campo: &str, msg: impl std::fmt::Display) {
        self.erros.push(format!("{campo}: {msg}"));
    }

    fn texto(&mut self, campo: &str, s: &str, min: usize, max: usize) {
        let n = s.chars().count();
        if s.trim().is_empty() {
            self.erro(campo, "não pode ficar vazio");
        } else if n < min || n > max {
            self.erro(campo, format!("deve ter de {min} a {max} caracteres (tem {n})"));
        }
        if s.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
            self.erro(campo, "contém caractere de controle");
        }
    }

    fn opcional_texto(&mut self, campo: &str, s: &Option<String>, min: usize, max: usize) {
        if let Some(s) = s {
            self.texto(campo, s, min, max);
        }
    }

    fn digitos(&mut self, campo: &str, s: &str, min: usize, max: usize) {
        if s.len() < min || s.len() > max || !s.bytes().all(|b| b.is_ascii_digit()) {
            let tamanho = if min == max {
                format!("{min}")
            } else {
                format!("{min} a {max}")
            };
            self.erro(campo, format!("deve ter {tamanho} dígitos"));
        }
    }

    fn opcional_telefone(&mut self, campo: &str, s: &Option<String>) {
        if let Some(s) = s {
            self.digitos(campo, s, 6, 20);
        }
    }

    fn serie(&mut self, campo: &str, s: &str) {
        // TSSerieDPS: [0-9]{1,4}|[0-8][0-9]{4} — séries 90000 a 99999 são reservadas.
        let ok = match s.len() {
            1..=4 => s.bytes().all(|b| b.is_ascii_digit()),
            5 => s.bytes().all(|b| b.is_ascii_digit()) && s.as_bytes()[0] != b'9',
            _ => false,
        };
        if !ok {
            self.erro(campo, "deve ter 1 a 5 dígitos (séries 90000 a 99999 são reservadas)");
        }
    }

    fn valor15(&mut self, campo: &str, v: Valor) {
        if v.centesimos() < 0 || v.parte_inteira() > 999_999_999_999_999 {
            self.erro(campo, "fora da faixa do leiaute");
        }
    }

    fn opcional_valor15(&mut self, campo: &str, v: Option<Valor>) {
        if let Some(v) = v {
            self.valor15(campo, v);
        }
    }

    /// `digitos_inteiros`: 1 para TSDec1V2 (até 9,99), 2 para TSDec2V2, 3 para TSDec3V2.
    fn percentual(&mut self, campo: &str, p: Percentual, digitos_inteiros: u32) {
        let max = 10_i64.pow(digitos_inteiros) - 1;
        if p.centesimos() < 0 || p.parte_inteira() > max {
            self.erro(campo, format!("deve estar entre 0 e {max}.99"));
        }
    }
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;

    /// DPS fictícia usada nos testes de todos os crates.
    pub fn dps_exemplo() -> Dps {
        serde_json::from_str(include_str!("../../../exemplos/dps-exemplo.json")).unwrap()
    }

    #[test]
    fn exemplo_valido() {
        let dps = dps_exemplo();
        assert_eq!(dps.validar(), Vec::<String>::new());
        assert_eq!(
            dps.id().unwrap().as_str(),
            "DPS351110221122233300018100001000000000000001"
        );
    }

    #[test]
    fn aponta_campos_invalidos() {
        let mut dps = dps_exemplo();
        dps.serie = "90000".into();
        dps.servico.codigo_tributacao_nacional = "17.19.01".into();
        dps.servico.descricao = "   ".into();
        dps.valores.issqn.aliquota = Some(Percentual::parse("10").unwrap());
        let erros = dps.validar();
        assert!(erros.iter().any(|e| e.starts_with("serie:")), "{erros:?}");
        assert!(
            erros
                .iter()
                .any(|e| e.starts_with("servico.codigo_tributacao_nacional:"))
        );
        assert!(erros.iter().any(|e| e.starts_with("servico.descricao:")));
        assert!(erros.iter().any(|e| e.starts_with("valores.issqn.aliquota:")));
    }

    #[test]
    fn total_de_tributos_conforme_o_simples() {
        let mut dps = dps_exemplo();
        let erro = |dps: &Dps| dps.validar().iter().any(|e| e.starts_with("valores.total_tributos:"));
        let simples = TotalTributos::PercentualSimples(Percentual::parse("11.48").unwrap());
        let percentuais = TotalTributos::Percentuais {
            federal: Percentual::parse("13.45").unwrap(),
            estadual: Percentual::parse("0").unwrap(),
            municipal: Percentual::parse("2").unwrap(),
        };
        for (situacao, total, recusa) in [
            (SituacaoSimples::MeEpp, TotalTributos::NaoInformado, true),
            (SituacaoSimples::MeEpp, simples.clone(), false),
            (SituacaoSimples::NaoOptante, TotalTributos::NaoInformado, true),
            (SituacaoSimples::NaoOptante, simples.clone(), true),
            (SituacaoSimples::NaoOptante, percentuais.clone(), false),
            (SituacaoSimples::Mei, simples, true),
            (SituacaoSimples::Mei, TotalTributos::NaoInformado, false),
        ] {
            dps.prestador.regime.simples = situacao;
            dps.prestador.regime.apuracao_simples =
                (situacao == SituacaoSimples::MeEpp).then_some(ApuracaoSimples::TudoNoSimples);
            dps.valores.total_tributos = total.clone();
            assert_eq!(erro(&dps), recusa, "{situacao:?} {total:?}");
        }
    }

    #[test]
    fn obra_obrigatoria_nos_codigos_de_construcao() {
        let mut dps = dps_exemplo();
        dps.servico.codigo_tributacao_nacional = "070201".into();
        let erro = |dps: &Dps| dps.validar().iter().any(|e| e.starts_with("servico.obra"));
        assert!(erro(&dps));
        dps.servico.obra = Some(Obra {
            inscricao_imobiliaria: None,
            identificacao: IdentificacaoObra::Cno("900112345678".into()),
        });
        assert!(!erro(&dps), "{:?}", dps.validar());
        dps.servico.obra.as_mut().unwrap().identificacao = IdentificacaoObra::Cib("123".into());
        assert!(erro(&dps));
    }

    #[test]
    fn ibs_cbs_obrigatorio_fora_do_simples() {
        let mut dps = dps_exemplo();
        dps.ibs_cbs = None;
        let erro = |dps: &Dps| dps.validar().iter().any(|e| e.starts_with("ibs_cbs:"));
        assert!(!erro(&dps), "Simples pode omitir");
        dps.prestador.regime.simples = SituacaoSimples::NaoOptante;
        dps.prestador.regime.apuracao_simples = None;
        assert!(erro(&dps));
        dps.ibs_cbs = Some(IbsCbs {
            consumo_pessoal: None,
            indicador_operacao: "100301".into(),
            destinatario_diferente_do_tomador: false,
            cst: "000".into(),
            classificacao_tributaria: "000001".into(),
        });
        assert!(!erro(&dps));
    }

    #[test]
    fn json_ida_e_volta() {
        let dps = dps_exemplo();
        let json = serde_json::to_string_pretty(&dps).unwrap();
        assert_eq!(serde_json::from_str::<Dps>(&json).unwrap(), dps);
    }
}
