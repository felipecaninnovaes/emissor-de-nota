//! Extração e formatação dos campos do DANFSe a partir do XML da NFS-e (NT 008/2026, item 2.4.5).
//!
//! Regra da NT: só se imprime o que está no XML; campo sem informação vira traço ("-").

use roxmltree::Node;

const NS: &str = "http://www.sped.fazenda.gov.br/nfse";
pub const TRACO: &str = "-";

#[derive(Debug, thiserror::Error)]
pub enum ErroDados {
    #[error("XML malformado: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("XML não é uma NFS-e do padrão nacional (falta {0})")]
    Estrutura(&'static str),
}

/// Prestador, tomador, destinatário ou intermediário, já formatados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parte {
    pub documento: String,
    pub inscricao_municipal: String,
    pub telefone: String,
    pub nome: String,
    pub municipio_uf: String,
    pub ibge_cep: String,
    pub endereco: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destinatario {
    ProprioTomador,
    NaoIdentificado,
    Outro(Parte),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issqn {
    pub tipo: String,
    pub municipio_incidencia: String,
    pub regime_especial: String,
    pub imunidade: String,
    pub suspensao: String,
    pub processo: String,
    pub beneficio: String,
    pub calculo_beneficio: String,
    pub deducoes: String,
    pub desconto_incondicionado: String,
    pub base_calculo: String,
    pub aliquota: String,
    pub retencao: String,
    pub apurado: String,
    /// Linhas da nota 5 (podem ser suprimidas se todos os campos estiverem vazios).
    pub mostrar_regime: bool,
    pub mostrar_beneficio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Federal {
    pub irrf: String,
    pub previdenciaria: String,
    pub sociais: String,
    pub pis: String,
    pub cofins: String,
    pub descricao_retidas: String,
    /// Nota 6: linha de PIS/COFINS só para competência até o fim de 2026.
    pub mostrar_pis_cofins: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbsCbs {
    pub cst_class: String,
    pub indicador_local: String,
    pub exclusoes: String,
    pub base_calculo: String,
    pub reducoes: String,
    pub aliquotas_ibs: String,
    pub aliq_efetiva_mun: String,
    pub valor_mun: String,
    pub aliq_efetiva_uf: String,
    pub valor_uf: String,
    pub total_ibs: String,
    pub aliquota_cbs: String,
    pub aliq_efetiva_cbs: String,
    pub valor_cbs: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totais {
    pub servico: String,
    pub desconto_incondicionado: String,
    pub desconto_condicionado: String,
    pub retencoes: String,
    pub liquido: String,
    pub total_ibs_cbs: String,
    pub liquido_com_ibs_cbs: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Danfse {
    pub homologacao: bool,
    /// "Município: Catanduva / SP" (omitido quando o item do código de tributação nacional é 99).
    pub municipio_cabecalho: Option<String>,
    pub ambiente_gerador: String,
    pub tipo_ambiente: String,

    pub chave: String,
    pub numero: String,
    pub competencia: String,
    pub emissao_nfse: String,
    pub numero_dps: String,
    pub serie_dps: String,
    pub emissao_dps: String,
    pub emitente: String,
    pub situacao: String,
    pub finalidade: String,

    pub prestador: Parte,
    pub simples_nacional: String,
    pub regime_apuracao_sn: String,
    pub tomador: Option<Parte>,
    pub destinatario: Destinatario,
    pub intermediario: Option<Parte>,

    pub codigo_tributacao: String,
    pub nbs: String,
    pub local_prestacao: String,
    pub descricao_codigo: String,
    pub descricao_servico: String,

    /// `None`: operação não sujeita ao ISSQN (nota 4).
    pub issqn: Option<Issqn>,
    pub federal: Federal,
    pub ibs_cbs: IbsCbs,
    pub totais: Totais,
    pub informacoes_complementares: String,
}

// ---------- navegação no XML ----------

fn filho<'a, 'i>(no: Node<'a, 'i>, nome: &str) -> Option<Node<'a, 'i>> {
    no.children()
        .find(|n| n.is_element() && n.tag_name().name() == nome && n.tag_name().namespace() == Some(NS))
}

fn em<'a, 'i>(no: Node<'a, 'i>, caminho: &[&str]) -> Option<Node<'a, 'i>> {
    caminho.iter().try_fold(no, |atual, nome| filho(atual, nome))
}

fn txt<'a>(no: Node<'a, '_>, caminho: &[&str]) -> Option<&'a str> {
    em(no, caminho)
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn ou_traco(v: Option<String>) -> String {
    v.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| TRACO.to_owned())
}

// ---------- formatação ----------

/// `v` tem exatamente `n` dígitos ASCII (só então é seguro fatiar por posição).
fn digitos(v: &str, n: usize) -> bool {
    v.len() == n && v.bytes().all(|b| b.is_ascii_digit())
}

pub fn cnpj(v: &str) -> String {
    // O CNPJ alfanumérico tem letras nas 12 primeiras posições.
    if v.len() != 14 || !v.is_ascii() {
        return v.to_owned();
    }
    format!("{}.{}.{}/{}-{}", &v[..2], &v[2..5], &v[5..8], &v[8..12], &v[12..])
}

pub fn cpf(v: &str) -> String {
    if !digitos(v, 11) {
        return v.to_owned();
    }
    format!("{}.{}.{}-{}", &v[..3], &v[3..6], &v[6..9], &v[9..])
}

pub fn cep(v: &str) -> String {
    if !digitos(v, 8) {
        return v.to_owned();
    }
    format!("{}.{}-{}", &v[..2], &v[2..5], &v[5..])
}

/// `2026-10-07` → `07/10/2026`.
pub fn data(v: &str) -> String {
    match (v.get(0..4), v.get(5..7), v.get(8..10)) {
        (Some(a), Some(m), Some(d)) => format!("{d}/{m}/{a}"),
        _ => v.to_owned(),
    }
}

/// `2026-10-07T14:05:05-03:00` → `07/10/2026 14:05:05` (hora como consta no XML).
pub fn data_hora(v: &str) -> String {
    match v.get(11..19) {
        Some(hora) => format!("{} {hora}", data(v)),
        None => data(v),
    }
}

/// `3248.00` → `R$ 3.248,00`.
pub fn moeda(v: &str) -> String {
    let (inteiro, frac) = v.split_once('.').unwrap_or((v, "00"));
    let (sinal, inteiro) = inteiro.strip_prefix('-').map_or(("", inteiro), |i| ("-", i));
    let numero = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if inteiro.is_empty() || !numero(inteiro) || !numero(frac) {
        return v.to_owned();
    }
    let frac = format!("{frac:0<2}");
    let mut com_milhar = String::new();
    for (i, c) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i) % 3 == 0 {
            com_milhar.push('.');
        }
        com_milhar.push(c);
    }
    format!("R$ {sinal}{com_milhar},{}", &frac[..2])
}

/// `2.5` → `2,50%`.
pub fn percentual(v: &str) -> String {
    let (inteiro, frac) = v.split_once('.').unwrap_or((v, "00"));
    format!("{inteiro},{:0<2}%", frac)
}

/// UF a partir dos dois primeiros dígitos do código IBGE.
pub fn uf_do_ibge(codigo: &str) -> Option<&'static str> {
    Some(match codigo.get(0..2)? {
        "11" => "RO",
        "12" => "AC",
        "13" => "AM",
        "14" => "RR",
        "15" => "PA",
        "16" => "AP",
        "17" => "TO",
        "21" => "MA",
        "22" => "PI",
        "23" => "CE",
        "24" => "RN",
        "25" => "PB",
        "26" => "PE",
        "27" => "AL",
        "28" => "SE",
        "29" => "BA",
        "31" => "MG",
        "32" => "ES",
        "33" => "RJ",
        "35" => "SP",
        "41" => "PR",
        "42" => "SC",
        "43" => "RS",
        "50" => "MS",
        "51" => "MT",
        "52" => "GO",
        "53" => "DF",
        _ => return None,
    })
}

fn codigo_trib_nac(v: &str) -> String {
    if digitos(v, 6) {
        format!("{}.{}.{}", &v[..2], &v[2..4], &v[4..])
    } else {
        v.to_owned()
    }
}

fn nbs(v: &str) -> String {
    if digitos(v, 9) {
        format!("{}.{}.{}.{}", &v[..1], &v[1..5], &v[5..7], &v[7..])
    } else {
        v.to_owned()
    }
}

fn descricao(opcoes: &[(&str, &str)], valor: Option<&str>) -> Option<String> {
    let v = valor?;
    Some(
        opcoes
            .iter()
            .find(|(c, _)| *c == v)
            .map_or_else(|| v.to_owned(), |(_, d)| (*d).to_owned()),
    )
}

/// Corta com reticências quando passa de `max` caracteres (regras "supere N caracteres").
pub fn limitar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        texto.to_owned()
    } else {
        let mut s: String = texto.chars().take(max.saturating_sub(3)).collect();
        s.push_str("...");
        s
    }
}

// ---------- montagem ----------

/// Nomes de municípios conhecidos pela própria NFS-e (o XML só traz o código dos demais).
struct Municipios<'a> {
    conhecidos: Vec<(&'a str, &'a str)>,
}

impl Municipios<'_> {
    fn nome_uf(&self, codigo: &str) -> String {
        let uf = uf_do_ibge(codigo).unwrap_or("");
        match self.conhecidos.iter().find(|(c, _)| *c == codigo) {
            Some((_, nome)) => format!("{nome} / {uf}"),
            None => format!("{codigo} / {uf}"),
        }
    }
}

fn documento(no: Node) -> Option<String> {
    txt(no, &["CNPJ"])
        .map(cnpj)
        .or_else(|| txt(no, &["CPF"]).map(cpf))
        .or_else(|| txt(no, &["NIF"]).map(str::to_owned))
        .or_else(|| txt(no, &["cNaoNIF"]).map(|_| "NIF não informado".to_owned()))
}

fn endereco_texto(end: Node) -> Option<String> {
    let partes: Vec<&str> = ["xLgr", "nro", "xCpl", "xBairro"]
        .iter()
        .filter_map(|t| txt(end, &[t]))
        .collect();
    (!partes.is_empty()).then(|| partes.join(", "))
}

fn parte(no: Node, municipios: &Municipios) -> Parte {
    let end = filho(no, "end");
    let nac = end.and_then(|e| filho(e, "endNac"));
    let ext = end.and_then(|e| filho(e, "endExt"));
    let municipio_uf = nac
        .and_then(|n| txt(n, &["cMun"]))
        .map(|c| municipios.nome_uf(c))
        .or_else(|| ext.and_then(|e| txt(e, &["xCidade"])).map(str::to_owned));
    let ibge_cep = match (nac, ext) {
        (Some(n), _) => Some(format!(
            "{} / {}",
            txt(n, &["cMun"]).unwrap_or(TRACO),
            txt(n, &["CEP"]).map_or(TRACO.into(), cep)
        )),
        (None, Some(e)) => txt(e, &["cEndPost"]).map(|c| format!("{c} (ext)")),
        _ => None,
    };
    Parte {
        documento: ou_traco(documento(no)),
        inscricao_municipal: ou_traco(txt(no, &["IM"]).map(str::to_owned)),
        telefone: ou_traco(txt(no, &["fone"]).map(str::to_owned)),
        nome: ou_traco(txt(no, &["xNome"]).map(|n| limitar(n, 80))),
        municipio_uf: ou_traco(municipio_uf.map(|m| limitar(&m, 37))),
        ibge_cep: ou_traco(ibge_cep),
        endereco: ou_traco(end.and_then(endereco_texto).map(|e| limitar(&e, 80))),
        email: ou_traco(txt(no, &["email"]).map(str::to_owned)),
    }
}

/// Prestador: a DPS emitida pelo próprio prestador não traz nome nem endereço; vêm do grupo `emit`.
fn prestador(prest: Node, emit: Option<Node>, municipios: &Municipios) -> Parte {
    let mut p = parte(prest, municipios);
    if let Some(emit) = emit {
        let vazio = |s: &str| s == TRACO;
        if vazio(&p.nome) {
            p.nome = ou_traco(txt(emit, &["xNome"]).map(|n| limitar(n, 80)));
        }
        if vazio(&p.inscricao_municipal) {
            p.inscricao_municipal = ou_traco(txt(emit, &["IM"]).map(str::to_owned));
        }
        if vazio(&p.telefone) {
            p.telefone = ou_traco(txt(emit, &["fone"]).map(str::to_owned));
        }
        if vazio(&p.email) {
            p.email = ou_traco(txt(emit, &["email"]).map(str::to_owned));
        }
        if let Some(end) = filho(emit, "enderNac") {
            if vazio(&p.endereco) {
                p.endereco = ou_traco(endereco_texto(end).map(|e| limitar(&e, 80)));
            }
            if vazio(&p.municipio_uf) {
                p.municipio_uf = ou_traco(txt(end, &["cMun"]).map(|c| municipios.nome_uf(c)));
            }
            if vazio(&p.ibge_cep) {
                p.ibge_cep = format!(
                    "{} / {}",
                    txt(end, &["cMun"]).unwrap_or(TRACO),
                    txt(end, &["CEP"]).map_or(TRACO.into(), cep)
                );
            }
        }
    }
    p
}

const EMITENTE: &[(&str, &str)] = &[("1", "Prestador"), ("2", "Tomador"), ("3", "Intermediário")];
const SITUACAO: &[(&str, &str)] = &[
    ("100", "NFS-e Gerada"),
    ("102", "NFS-e de Decisão Judicial"),
    ("103", "NFS-e Avulsa"),
    ("107", "NFS-e MEI"),
];
const AMB_GER: &[(&str, &str)] = &[("1", "Prefeitura"), ("2", "Sistema Nacional da NFS-e")];
const TP_AMB: &[(&str, &str)] = &[("1", "Produção"), ("2", "Homologação")];
const FINALIDADE: &[(&str, &str)] = &[("0", "NFS-e regular")];
const SIMPLES: &[(&str, &str)] = &[
    ("1", "Não Optante"),
    ("2", "Optante - Microempreendedor Individual (MEI)"),
    ("3", "Optante - Microempresa ou Empresa de Pequeno Porte (ME/EPP)"),
];
const APURACAO_SN: &[(&str, &str)] = &[
    (
        "1",
        "Regime de apuração dos tributos federais e municipal pelo Simples Nacional",
    ),
    (
        "2",
        "Regime de apuração dos tributos federais pelo SN e o ISSQN pela NFS-e conforme respectiva legislação municipal do tributo",
    ),
    (
        "3",
        "Regime de apuração dos tributos federais e municipal pela NFS-e conforme respectivas legislações federal e municipal de cada tributo",
    ),
];
const TRIB_ISSQN: &[(&str, &str)] = &[
    ("1", "Operação Tributável"),
    ("2", "Imunidade"),
    ("3", "Exportação de serviço"),
    ("4", "Não Incidência"),
];
const REG_ESP: &[(&str, &str)] = &[
    ("0", "Nenhum"),
    ("1", "Ato Cooperado (Cooperativa)"),
    ("2", "Estimativa"),
    ("3", "Microempresa Municipal"),
    ("4", "Notário ou Registrador"),
    ("5", "Profissional Autônomo"),
    ("6", "Sociedade de Profissionais"),
    ("9", "Outros"),
];
const IMUNIDADE: &[(&str, &str)] = &[
    ("0", "Imunidade (tipo não informado na nota de origem)"),
    ("1", "Patrimônio, renda ou serviços, uns dos outros"),
    ("2", "Templos de qualquer culto"),
    (
        "3",
        "Patrimônio, renda ou serviços dos partidos políticos, entidades sindicais, educação e assistência social",
    ),
    ("4", "Livros, jornais, periódicos e o papel destinado a sua impressão"),
    ("5", "Fonogramas e videofonogramas musicais produzidos no Brasil"),
];
const SUSPENSAO: &[(&str, &str)] = &[
    ("1", "Exigibilidade Suspensa por Decisão Judicial"),
    ("2", "Exigibilidade Suspensa por Processo Administrativo"),
];
const BENEFICIO: &[(&str, &str)] = &[
    ("1", "Isenção"),
    ("2", "Redução da BC em percentual"),
    ("3", "Redução da BC em valor"),
    ("4", "Alíquota diferenciada"),
];
const RETENCAO_ISSQN: &[(&str, &str)] = &[
    ("1", "Não Retido"),
    ("2", "Retido pelo Tomador"),
    ("3", "Retido pelo Intermediário"),
];
const RETENCAO_PIS: &[(&str, &str)] = &[
    ("0", "PIS/COFINS/CSLL Não Retidos"),
    ("1", "PIS/COFINS Retidos"),
    ("2", "PIS/COFINS Não Retidos"),
    ("3", "PIS/COFINS/CSLL Retidos"),
    ("4", "PIS/COFINS Retidos, CSLL Não Retido"),
    ("5", "PIS Retido, COFINS/CSLL Não Retido"),
    ("6", "COFINS Retido, PIS/CSLL Não Retido"),
    ("7", "PIS Não Retido, COFINS/CSLL Retidos"),
    ("8", "PIS/COFINS Não Retidos, CSLL Retido"),
    ("9", "COFINS Não Retido, PIS/CSLL Retidos"),
];

fn valor(no: Option<Node>, caminho: &[&str]) -> String {
    ou_traco(no.and_then(|n| txt(n, caminho)).map(moeda))
}

fn perc(no: Option<Node>, caminho: &[&str]) -> String {
    ou_traco(no.and_then(|n| txt(n, caminho)).map(percentual))
}

fn totais_aproximados(trib: Option<Node>) -> String {
    let prefixo = "Totais Aproximados dos Tributos cfe. Lei nº 12.741/2012:";
    let tot = trib.and_then(|t| filho(t, "totTrib"));
    if let Some(v) = tot.and_then(|t| filho(t, "vTotTrib")) {
        return format!(
            "{prefixo} Federais: {}; Estaduais: {}; Municipais: {}",
            valor(Some(v), &["vTotTribFed"]),
            valor(Some(v), &["vTotTribEst"]),
            valor(Some(v), &["vTotTribMun"])
        );
    }
    if let Some(p) = tot.and_then(|t| filho(t, "pTotTrib")) {
        return format!(
            "{prefixo} Federais: {}; Estaduais: {}; Municipais: {}",
            perc(Some(p), &["pTotTribFed"]),
            perc(Some(p), &["pTotTribEst"]),
            perc(Some(p), &["pTotTribMun"])
        );
    }
    if let Some(sn) = tot.and_then(|t| txt(t, &["pTotTribSN"])) {
        return format!("{prefixo} Simples Nacional: {}", percentual(sn));
    }
    format!("{prefixo} não informados (Decreto 8.264/2014)")
}

pub fn ler(xml: &str) -> Result<Danfse, ErroDados> {
    let doc = roxmltree::Document::parse(xml)?;
    let inf = doc
        .descendants()
        .find(|n| n.tag_name().name() == "infNFSe" && n.tag_name().namespace() == Some(NS))
        .ok_or(ErroDados::Estrutura("infNFSe"))?;
    let dps = em(inf, &["DPS", "infDPS"]).ok_or(ErroDados::Estrutura("DPS/infDPS"))?;
    let emit = filho(inf, "emit");
    // Toda NFS-e tem `prest`; na falta, os dados do emitente bastam para o cabeçalho do prestador.
    let prest = filho(dps, "prest").or(emit).ok_or(ErroDados::Estrutura("prest"))?;
    let serv = filho(dps, "serv");
    let valores_dps = filho(dps, "valores");
    let trib = valores_dps.and_then(|v| filho(v, "trib"));
    let trib_mun = trib.and_then(|t| filho(t, "tribMun"));
    let trib_fed = trib.and_then(|t| filho(t, "tribFed"));
    let piscofins = trib_fed.and_then(|t| filho(t, "piscofins"));
    let valores_nfse = filho(inf, "valores");
    let desc = valores_dps.and_then(|v| filho(v, "vDescCondIncond"));
    let ibscbs_dps = filho(dps, "IBSCBS");
    let ibscbs_nfse = filho(inf, "IBSCBS");

    let mut conhecidos = Vec::new();
    for (c, n) in [
        ("cLocEmi", "xLocEmi"),
        ("cLocPrestacao", "xLocPrestacao"),
        ("cLocIncid", "xLocIncid"),
    ] {
        let codigo = txt(dps, &[c])
            .or_else(|| txt(inf, &[c]))
            .or_else(|| serv.and_then(|s| txt(s, &["locPrest", c])));
        if let (Some(codigo), Some(nome)) = (codigo, txt(inf, &[n])) {
            conhecidos.push((codigo, nome));
        }
    }
    let municipios = Municipios { conhecidos };

    let c_trib_nac = serv.and_then(|s| txt(s, &["cServ", "cTribNac"]));
    let uf_emitente = emit
        .and_then(|e| txt(e, &["enderNac", "UF"]))
        .or_else(|| txt(dps, &["cLocEmi"]).and_then(uf_do_ibge));
    let municipio_cabecalho = if c_trib_nac.is_some_and(|c| c.starts_with("99")) {
        None
    } else {
        txt(inf, &["xLocEmi"]).map(|m| limitar(&format!("Município: {m} / {}", uf_emitente.unwrap_or("")), 37))
    };

    let competencia = txt(dps, &["dCompet"]);
    let ano_competencia: i32 = competencia
        .and_then(|c| c.get(0..4))
        .and_then(|a| a.parse().ok())
        .unwrap_or(0);

    let tomador = filho(dps, "toma").map(|t| parte(t, &municipios));
    let destinatario = match ibscbs_dps.and_then(|i| filho(i, "dest")) {
        Some(d) => Destinatario::Outro(parte(d, &municipios)),
        None if tomador.is_some() => Destinatario::ProprioTomador,
        None => Destinatario::NaoIdentificado,
    };

    let local_prestacao = serv.and_then(|s| filho(s, "locPrest")).map(|l| {
        match (txt(l, &["cLocPrestacao"]), txt(l, &["cPaisPrestacao"])) {
            (Some(c), _) => format!("{} / BR", municipios.nome_uf(c)),
            (None, Some(p)) => p.to_owned(),
            _ => TRACO.to_owned(),
        }
    });

    let trib_issqn = trib_mun.and_then(|t| txt(t, &["tribISSQN"]));
    let issqn = (trib_issqn != Some("4")).then(|| {
        let incidencia = txt(inf, &["cLocIncid"])
            .map(|c| format!("{} / BR", municipios.nome_uf(c)))
            .or_else(|| trib_mun.and_then(|t| txt(t, &["cPaisResult"])).map(str::to_owned));
        let reg_esp = txt(prest, &["regTrib", "regEspTrib"]);
        let imunidade = trib_mun.and_then(|t| txt(t, &["tpImunidade"]));
        let susp = trib_mun.and_then(|t| filho(t, "exigSusp"));
        let beneficio = valores_nfse.and_then(|v| txt(v, &["tpBM"]));
        let calc_bm = valores_nfse
            .and_then(|v| txt(v, &["vCalcBM"]))
            .or_else(|| trib_mun.and_then(|t| txt(t, &["BM", "vRedBCBM"])));
        let deducoes = valores_nfse
            .and_then(|v| txt(v, &["vCalcAjusteBCISSQN"]).or_else(|| txt(v, &["vCalcDR"])))
            .or_else(|| {
                valores_dps
                    .and_then(|v| txt(v, &["vAjusteBC", "vAjusteBCISSQN"]).or_else(|| txt(v, &["vDedRed", "vDR"])))
            });
        let desc_incond = desc.and_then(|d| txt(d, &["vDescIncond"]));
        Issqn {
            tipo: ou_traco(descricao(TRIB_ISSQN, trib_issqn)),
            municipio_incidencia: ou_traco(incidencia.map(|m| limitar(&m, 42))),
            regime_especial: ou_traco(descricao(REG_ESP, reg_esp)),
            imunidade: ou_traco(descricao(IMUNIDADE, imunidade).map(|d| limitar(&d, 40))),
            suspensao: ou_traco(descricao(SUSPENSAO, susp.and_then(|s| txt(s, &["tpSusp"]))).map(|d| limitar(&d, 40))),
            processo: ou_traco(susp.and_then(|s| txt(s, &["nProcesso"])).map(str::to_owned)),
            beneficio: ou_traco(descricao(BENEFICIO, beneficio)),
            calculo_beneficio: ou_traco(calc_bm.map(moeda)),
            deducoes: ou_traco(deducoes.map(moeda)),
            desconto_incondicionado: ou_traco(desc_incond.map(moeda)),
            base_calculo: valor(valores_nfse, &["vBC"]),
            aliquota: perc(valores_nfse, &["pAliqAplic"]),
            retencao: ou_traco(descricao(
                RETENCAO_ISSQN,
                trib_mun.and_then(|t| txt(t, &["tpRetISSQN"])),
            )),
            apurado: valor(valores_nfse, &["vISSQN"]),
            mostrar_regime: reg_esp.is_some_and(|r| r != "0") || imunidade.is_some() || susp.is_some(),
            mostrar_beneficio: beneficio.is_some() || calc_bm.is_some() || deducoes.is_some() || desc_incond.is_some(),
        }
    });

    let federal = Federal {
        irrf: valor(trib_fed, &["vRetIRRF"]),
        previdenciaria: valor(trib_fed, &["vRetCP"]),
        sociais: valor(trib_fed, &["vRetCSLL"]),
        pis: valor(piscofins, &["vPis"]),
        cofins: valor(piscofins, &["vCofins"]),
        descricao_retidas: ou_traco(descricao(
            RETENCAO_PIS,
            piscofins.and_then(|p| txt(p, &["tpRetPisCofins"])),
        )),
        mostrar_pis_cofins: ano_competencia <= 2026,
    };

    // NT 009 subiu CST e cClassTrib para `trib`; antes ficavam em `trib/gIBSCBS`.
    let g = ibscbs_dps.and_then(|i| em(i, &["valores", "trib", "gIBSCBS"]).or_else(|| em(i, &["valores", "trib"])));
    let iv = ibscbs_nfse.and_then(|i| filho(i, "valores"));
    let tot = ibscbs_nfse.and_then(|i| filho(i, "totCIBS"));
    let par = |a: String, b: String| {
        if a == TRACO && b == TRACO {
            TRACO.to_owned()
        } else {
            format!("{a} / {b}")
        }
    };
    let ibs_cbs = IbsCbs {
        cst_class: match (
            g.and_then(|g| txt(g, &["CST"])),
            g.and_then(|g| txt(g, &["cClassTrib"])),
        ) {
            (Some(c), Some(k)) => format!("{c} / {k}"),
            _ => TRACO.to_owned(),
        },
        indicador_local: match ibscbs_dps.and_then(|i| txt(i, &["cIndOp"])) {
            Some(op) => {
                let cod = ibscbs_nfse.and_then(|i| txt(i, &["cLocalidadeIncid"])).unwrap_or(TRACO);
                let nome = ibscbs_nfse.and_then(|i| txt(i, &["xLocalidadeIncid"])).unwrap_or(TRACO);
                limitar(
                    &format!("{op} / {cod} / {nome} / {}", uf_do_ibge(cod).unwrap_or(TRACO)),
                    56,
                )
            }
            None => TRACO.to_owned(),
        },
        exclusoes: TRACO.to_owned(),
        base_calculo: valor(iv, &["vBC"]),
        reducoes: {
            let r = [
                iv.and_then(|v| txt(v, &["uf", "pRedAliqUF"])),
                iv.and_then(|v| txt(v, &["mun", "pRedAliqMun"])),
                iv.and_then(|v| txt(v, &["fed", "pRedAliqCBS"])),
            ];
            if r.iter().all(Option::is_none) {
                TRACO.to_owned()
            } else {
                r.iter()
                    .map(|p| p.map_or(TRACO.to_owned(), percentual))
                    .collect::<Vec<_>>()
                    .join(" / ")
            }
        },
        aliquotas_ibs: par(perc(iv, &["uf", "pIBSUF"]), perc(iv, &["mun", "pIBSMun"])),
        aliq_efetiva_mun: perc(iv, &["mun", "pAliqEfetMun"]),
        valor_mun: valor(tot, &["gIBS", "gIBSMunTot", "vIBSMun"]),
        aliq_efetiva_uf: perc(iv, &["uf", "pAliqEfetUF"]),
        valor_uf: valor(tot, &["gIBS", "gIBSUFTot", "vIBSUF"]),
        total_ibs: valor(tot, &["gIBS", "vIBSTot"]),
        aliquota_cbs: perc(iv, &["fed", "pCBS"]),
        aliq_efetiva_cbs: perc(iv, &["fed", "pAliqEfetCBS"]),
        valor_cbs: valor(tot, &["gCBS", "vCBS"]),
    };

    let total_ibs_cbs = match (
        tot.and_then(|t| txt(t, &["gIBS", "vIBSTot"])),
        tot.and_then(|t| txt(t, &["gCBS", "vCBS"])),
    ) {
        (None, None) => TRACO.to_owned(),
        (a, b) => {
            let centavos = |v: Option<&str>| -> i64 {
                v.map_or(0, |v| {
                    let (i, f) = v.split_once('.').unwrap_or((v, "0"));
                    let f = format!("{f:0<2}");
                    i.parse::<i64>().unwrap_or(0) * 100 + f.get(..2).and_then(|f| f.parse().ok()).unwrap_or(0)
                })
            };
            let soma = centavos(a) + centavos(b);
            let sinal = if soma < 0 { "-" } else { "" };
            let soma = soma.unsigned_abs();
            moeda(&format!("{sinal}{}.{:02}", soma / 100, soma % 100))
        }
    };
    let totais = Totais {
        servico: valor(valores_dps, &["vServPrest", "vServ"]),
        desconto_incondicionado: valor(desc, &["vDescIncond"]),
        desconto_condicionado: valor(desc, &["vDescCond"]),
        retencoes: valor(valores_nfse, &["vTotalRet"]),
        liquido: valor(valores_nfse, &["vLiq"]),
        total_ibs_cbs,
        liquido_com_ibs_cbs: valor(tot, &["vTotNF"]),
    };

    let info = serv.and_then(|s| filho(s, "infoCompl"));
    let mut complementares: Vec<String> = Vec::new();
    let mut add = |rotulo: &str, v: Option<&str>| {
        if let Some(v) = v {
            complementares.push(format!("{rotulo} {v}"));
        }
    };
    add("Inf. Cont.:", info.and_then(|i| txt(i, &["xInfComp"])));
    add("NFS-e Subst.:", txt(dps, &["subst", "chSubstda"]));
    add("Doc. Ref.:", info.and_then(|i| txt(i, &["docRef"])));
    add("Cod. Obra:", serv.and_then(|s| txt(s, &["obra", "cObra"])));
    add(
        "Insc. Imob.:",
        serv.and_then(|s| txt(s, &["obra", "inscImobFisc"]))
            .or_else(|| ibscbs_dps.and_then(|i| txt(i, &["imovel", "inscImobFisc"]))),
    );
    add("Cod. Evt.:", serv.and_then(|s| txt(s, &["atvEvento", "idAtvEvt"])));
    add("Doc. Tec.:", info.and_then(|i| txt(i, &["idDocTec"])));
    add("Núm. Ped.:", info.and_then(|i| txt(i, &["xPed"])));
    let itens: Vec<&str> = info
        .and_then(|i| filho(i, "gItemPed"))
        .map(|g| {
            g.children()
                .filter(|n| n.is_element())
                .filter_map(|n| n.text())
                .collect()
        })
        .unwrap_or_default();
    let itens = itens.join(", ");
    add("Item Ped.:", (!itens.is_empty()).then_some(itens.as_str()));
    add("Inf. A. T. Mun.:", txt(inf, &["xOutInf"]));
    let totais_tributos = totais_aproximados(trib);
    let resto = limitar(
        &complementares.join(" | "),
        1997usize.saturating_sub(totais_tributos.chars().count() + 3),
    );
    let informacoes_complementares = if resto.is_empty() {
        totais_tributos
    } else {
        format!("{resto} | {totais_tributos}")
    };

    let tp_amb = txt(dps, &["tpAmb"]);
    Ok(Danfse {
        homologacao: tp_amb == Some("2"),
        municipio_cabecalho,
        ambiente_gerador: ou_traco(descricao(AMB_GER, txt(inf, &["ambGer"]))),
        tipo_ambiente: ou_traco(descricao(TP_AMB, tp_amb)),
        chave: ou_traco(
            inf.attribute("Id")
                .and_then(|i| i.strip_prefix("NFS"))
                .map(str::to_owned),
        ),
        numero: ou_traco(txt(inf, &["nNFSe"]).map(str::to_owned)),
        competencia: ou_traco(competencia.map(data)),
        emissao_nfse: ou_traco(txt(inf, &["dhProc"]).map(data_hora)),
        numero_dps: ou_traco(txt(dps, &["nDPS"]).map(str::to_owned)),
        serie_dps: ou_traco(txt(dps, &["serie"]).map(str::to_owned)),
        emissao_dps: ou_traco(txt(dps, &["dhEmi"]).map(data_hora)),
        emitente: ou_traco(descricao(EMITENTE, txt(dps, &["tpEmit"]))),
        situacao: ou_traco(descricao(SITUACAO, txt(inf, &["cStat"])).map(|s| limitar(&s, 40))),
        finalidade: ou_traco(
            descricao(
                FINALIDADE,
                txt(dps, &["finNFSe"]).or_else(|| ibscbs_dps.and_then(|i| txt(i, &["finNFSe"]))),
            )
            .map(|s| limitar(&s, 40)),
        ),
        prestador: prestador(prest, emit, &municipios),
        simples_nacional: ou_traco(descricao(SIMPLES, txt(prest, &["regTrib", "opSimpNac"])).map(|s| limitar(&s, 40))),
        regime_apuracao_sn: ou_traco(
            descricao(APURACAO_SN, txt(prest, &["regTrib", "regApTribSN"])).map(|s| limitar(&s, 80)),
        ),
        tomador,
        destinatario,
        intermediario: filho(dps, "interm").map(|i| parte(i, &municipios)),
        codigo_tributacao: ou_traco(c_trib_nac.map(|c| {
            let mut s = codigo_trib_nac(c);
            if let Some(m) = serv.and_then(|s| txt(s, &["cServ", "cTribMun"])) {
                s.push_str(&format!(" / {m}"));
            }
            s
        })),
        nbs: ou_traco(serv.and_then(|s| txt(s, &["cServ", "cNBS"])).map(nbs)),
        local_prestacao: ou_traco(local_prestacao.map(|l| limitar(&l, 42))),
        descricao_codigo: ou_traco(
            txt(inf, &["xTribMun"])
                .or_else(|| txt(inf, &["xTribNac"]))
                .map(|d| limitar(d, 170)),
        ),
        descricao_servico: ou_traco(
            serv.and_then(|s| txt(s, &["cServ", "xDescServ"]))
                .map(|d| limitar(d, 1300)),
        ),
        issqn,
        federal,
        ibs_cbs,
        totais,
        informacoes_complementares,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn texto_fora_do_formato_nao_derruba() {
        // Multibyte na posição do corte: antes fatiava por byte no meio do caractere.
        assert_eq!(cnpj("1é222333000181"), "1é222333000181");
        assert_eq!(cpf("5é998224725"), "5é998224725");
        assert_eq!(cep("1é800000"), "1é800000");
        assert_eq!(nbs("1é2345678"), "1é2345678");
        assert_eq!(moeda("10.é"), "10.é");
        assert_eq!(moeda("abc"), "abc");
        assert_eq!(moeda("-0.05"), "R$ -0,05");
    }

    #[test]
    fn formatos() {
        assert_eq!(cnpj("11222333000181"), "11.222.333/0001-81");
        assert_eq!(cpf("52998224725"), "529.982.247-25");
        assert_eq!(cep("15800000"), "15.800-000");
        assert_eq!(moeda("3248.00"), "R$ 3.248,00");
        assert_eq!(moeda("1234567.5"), "R$ 1.234.567,50");
        assert_eq!(moeda("0.07"), "R$ 0,07");
        assert_eq!(percentual("11.48"), "11,48%");
        assert_eq!(percentual("2"), "2,00%");
        assert_eq!(data_hora("2026-10-07T14:05:05-03:00"), "07/10/2026 14:05:05");
        assert_eq!(codigo_trib_nac("171901"), "17.19.01");
        assert_eq!(nbs("113022100"), "1.1302.21.00");
        assert_eq!(uf_do_ibge("3511102"), Some("SP"));
        assert_eq!(limitar("abcdef", 5), "ab...");
    }
}
