//! Montagem da DPS a partir dos cadastros: retenções, variáveis da descrição e data de emissão.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Utc};

use nfse_core::modelo::{Documento, LeiauteDps, OperacaoGovernamental, Prestador, Servico as ServicoDps, Valores};
use nfse_core::{Dps, Valor};
use nfse_db::empresas::Empresa;
use nfse_db::servicos::Servico;
use nfse_db::tomadores::TomadorVinculado;

use crate::{PedidoEmissao, ReterNota, VERSAO_APLICATIVO};

/// `tpOper` 1: fornecimento com pagamento posterior, o caso do serviço prestado a órgão público.
const PAGAMENTO_POSTERIOR: u8 = 1;

/// Tributação da nota: tira do serviço as retenções que só valem para tomador que retém e, para
/// estado ou município, as que ele não faz (CSLL, PIS e COFINS).
pub fn tributacao_da_nota(
    servico: &Servico,
    tomador: Option<&TomadorVinculado>,
    reter: ReterNota,
) -> nfse_db::servicos::Tributacao {
    let (federais, iss) = reter.efetiva(tomador);
    let mut t = servico.tributacao.clone();
    if !t.retencoes.aplicar.vale(federais) {
        t.retencoes = Default::default();
        t.federais.retencao_irrf = None;
        t.federais.retencao_csll = None;
        t.federais.retencao_cp = None;
        if let Some(pc) = t.federais.pis_cofins.as_mut() {
            pc.tipo_retencao = pc.tipo_retencao.map(|_| 0);
        }
    }
    // Estado, DF e município retêm o IRRF, mas não a CSLL, o PIS nem a COFINS (sem convênio com a RFB).
    if tomador.is_some_and(|t| !t.tipo.retem_contribuicoes()) {
        t.retencoes.csll = None;
        t.retencoes.pis = None;
        t.retencoes.cofins = None;
        t.federais.retencao_csll = None;
        if let Some(pc) = t.federais.pis_cofins.as_mut() {
            pc.tipo_retencao = pc.tipo_retencao.map(|_| 0);
        }
    }
    if !t.aplicar_retencao_iss.vale(iss) {
        t.issqn.retencao = nfse_core::modelo::RetencaoIssqn::NaoRetido;
    }
    t
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

const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// Variáveis aceitas nas descrições, na ordem em que a tela as oferece.
pub const VARIAVEIS_DESCRICAO: [&str; 9] = [
    "mes_ref",
    "mes_ref_extenso",
    "ano_ref",
    "competencia",
    "data_emissao",
    "tomador",
    "empresa",
    "valor",
    "valor_liquido",
];

/// Troca as variáveis do mês de referência: `{mes_ref}` (09/2026), `{mes_ref_extenso}` (setembro de
/// 2026) e `{ano_ref}`. As demais ficam para `montar_dps`, que conhece a nota inteira.
pub fn aplicar_mes_ref(texto: &str, mes: NaiveDate) -> String {
    texto
        .replace("{mes_ref}", &format!("{:02}/{}", mes.month(), mes.year()))
        .replace(
            "{mes_ref_extenso}",
            &format!("{} de {}", MESES[mes.month0() as usize], mes.year()),
        )
        .replace("{ano_ref}", &mes.year().to_string())
}

/// Valor menos as retenções federais e o ISS retido. O ISS só é conhecido quando o serviço traz a
/// alíquota (valor × alíquota); sem ela quem calcula é a Sefin e aqui não há como saber.
pub(crate) fn valor_liquido(
    valor: Valor,
    federais: &nfse_core::modelo::TributosFederais,
    t: &nfse_db::servicos::Tributacao,
) -> Option<Valor> {
    let mut retido: i64 = [federais.retencao_irrf, federais.retencao_cp, federais.retencao_csll]
        .into_iter()
        .flatten()
        .map(|v| v.centesimos())
        .sum();
    if t.issqn.retencao != nfse_core::modelo::RetencaoIssqn::NaoRetido {
        retido += valor.aplicar(t.issqn.aliquota?).centesimos();
    }
    Some(Valor::de_centesimos(valor.centesimos() - retido))
}

/// Mensagem para a descrição com variáveis que não existem.
pub fn texto_variaveis_desconhecidas(nomes: &[String]) -> String {
    if nomes == ["valor_liquido"] {
        return "{valor_liquido} não pode ser calculado: o ISS é retido e o serviço não tem a alíquota do ISS \
                (a Sefin é quem calcula). Informe a alíquota no serviço ou tire a variável."
            .into();
    }
    let lista: Vec<String> = nomes.iter().map(|n| format!("{{{n}}}")).collect();
    format!(
        "a descrição tem variável desconhecida: {} (use {})",
        lista.join(", "),
        VARIAVEIS_DESCRICAO.map(|v| format!("{{{v}}}")).join(", ")
    )
}

/// Valor no formato brasileiro, sem símbolo: `1.250,00`.
pub(crate) fn valor_br(v: Valor) -> String {
    let c = v.centesimos();
    let sinal = if c < 0 { "-" } else { "" };
    let c = c.unsigned_abs();
    let inteiro = (c / 100).to_string();
    let mut milhares = String::new();
    for (i, d) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i).is_multiple_of(3) {
            milhares.push('.');
        }
        milhares.push(d);
    }
    format!("{sinal}{milhares},{:02}", c % 100)
}

/// Variáveis `{nome}` que sobraram no texto (digitadas errado ou inexistentes).
pub fn variaveis_desconhecidas(texto: &str) -> Vec<String> {
    let mut achadas = Vec::new();
    let mut resto = texto;
    while let Some(ini) = resto.find('{') {
        let depois = &resto[ini + 1..];
        let Some(fim) = depois.find('}') else { break };
        let nome = &depois[..fim];
        if !nome.is_empty()
            && nome
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && !achadas.iter().any(|a| a == nome)
        {
            achadas.push(nome.to_owned());
        }
        resto = depois;
    }
    achadas
}

pub(crate) fn mes_anterior(data: NaiveDate) -> NaiveDate {
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
    // NT 007/2026: PIS, COFINS e CSLL retidos vão somados em vRetCSLL (o tpRetPisCofins diz quais
    // entram); vPis/vCofins são débito de apuração própria e não entram no vTotalRet nem no vLiq.
    let retidos: Vec<Valor> = [r.csll, r.pis, r.cofins].into_iter().filter_map(aplicar).collect();
    if !retidos.is_empty() {
        f.retencao_csll = Some(Valor::de_centesimos(retidos.iter().map(|v| v.centesimos()).sum()));
    }
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
        // Retido não é débito próprio: vPis/vCofins saem da base do IBS/CBS na Sefin, e repetir
        // aqui o que já está em vRetCSLL reduz a base duas vezes (nota real: base = valor − ISS).
        pc.base_calculo = None;
        pc.aliquota_pis = None;
        pc.aliquota_cofins = None;
        pc.valor_pis = None;
        pc.valor_cofins = None;
    } else if let Some(pc) = f.pis_cofins.as_mut()
        && matches!(pc.cst.as_str(), "01" | "02")
        && (pc.aliquota_pis.is_some() || pc.aliquota_cofins.is_some())
    {
        // Apuração própria: o Emissor Web manda base, alíquotas e valores (truncados no centavo) e a
        // Sefin desconta vPis e vCofins da base do IBS/CBS; sem eles a base fica só menos o ISS.
        pc.base_calculo = Some(valor);
        pc.valor_pis = pc.aliquota_pis.map(|p| valor.aplicar_truncado(p));
        pc.valor_cofins = pc.aliquota_cofins.map(|p| valor.aplicar_truncado(p));
        // Retidos pelo tipo do cadastro (sem percentual de retenção): como no Portal Nacional, vPis e
        // vCofins seguem na DPS e também entram na soma de vRetCSLL.
        let (pis_retido, cofins_retida) = match pc.tipo_retencao {
            Some(1 | 3 | 4) => (true, true),
            Some(5 | 9) => (true, false),
            Some(6 | 7) => (false, true),
            _ => (false, false),
        };
        let retidos = [(pis_retido, pc.valor_pis), (cofins_retida, pc.valor_cofins)]
            .into_iter()
            .filter_map(|(retido, v)| v.filter(|_| retido))
            .map(|v| v.centesimos())
            .sum::<i64>();
        if retidos > 0 {
            let csll = f.retencao_csll.map_or(0, |v| v.centesimos());
            f.retencao_csll = Some(Valor::de_centesimos(csll + retidos));
        }
    }
    // Com retenção percentual no serviço, o tipo sai dela (e não do que ficou no cadastro).
    if (r.pis.is_some() || r.cofins.is_some() || r.csll.is_some())
        && let Some(pc) = f.pis_cofins.as_mut()
    {
        pc.tipo_retencao = Some(tipo_retencao_pis_cofins(
            r.pis.is_some(),
            r.cofins.is_some(),
            r.csll.is_some(),
        ));
    }
    f
}

/// `tpRetPisCofins` para a combinação de tributos retidos.
fn tipo_retencao_pis_cofins(pis: bool, cofins: bool, csll: bool) -> u8 {
    match (pis, cofins, csll) {
        (false, false, false) => 0,
        (true, true, true) => 3,
        (true, true, false) => 4,
        (true, false, false) => 5,
        (false, true, false) => 6,
        (false, true, true) => 7,
        (false, false, true) => 8,
        (true, false, true) => 9,
    }
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
    let t = &tributacao_da_nota(servico, tomador, pedido.reter);
    let federais = federais_da_nota(t, pedido.valor);
    let mut descricao = aplicar_mes_ref(pedido.descricao.as_deref().unwrap_or(&servico.descricao), mes)
        .replace("{competencia}", &emissao.format("%m/%Y").to_string())
        .replace("{data_emissao}", &emissao.format("%d/%m/%Y").to_string())
        .replace("{tomador}", tomador.map_or("", |t| t.tomador.nome.as_str()))
        .replace("{empresa}", &empresa.nome)
        .replace("{valor}", &valor_br(pedido.valor));
    // Sem como calcular (ISS retido sem alíquota no serviço), a variável fica e a emissão é barrada.
    if let Some(liquido) = valor_liquido(pedido.valor, &federais, t) {
        descricao = descricao.replace("{valor_liquido}", &valor_br(liquido));
    }
    Dps {
        ambiente: empresa.ambiente,
        leiaute: LeiauteDps::padrao(empresa.ambiente),
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
            obra: pedido.obra.clone(),
            informacoes_complementares: pedido.informacoes_complementares.clone(),
        },
        valores: Valores {
            servico: pedido.valor,
            desconto_incondicionado: None,
            desconto_condicionado: None,
            issqn: t.issqn.clone(),
            federais,
            total_tributos: t.total_tributos.clone(),
        },
        ibs_cbs: t.ibs_cbs.clone().map(|mut i| {
            i.governamental = tomador
                .and_then(|t| t.tipo.ente_governamental())
                .map(|ente| OperacaoGovernamental {
                    ente,
                    tipo_operacao: PAGAMENTO_POSTERIOR,
                });
            i
        }),
    }
}
