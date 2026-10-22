//! Serialização da DPS na ordem exata do `xs:sequence` do leiaute 1.01.
//!
//! O XML sai sem espaços entre elementos e sem a assinatura; assinar depois com
//! [`crate::assinar`] usando o Id devolvido.

use nfse_core::modelo::{Documento, EnderecoNacional, IdentificacaoObra, LeiauteDps, SituacaoSimples, TotalTributos};
use nfse_core::{Dps, DpsId};

pub const NS_NFSE: &str = "http://www.sped.fazenda.gov.br/nfse";

#[derive(Debug, thiserror::Error)]
#[error("DPS inválida:\n- {}", .0.join("\n- "))]
pub struct DpsInvalida(pub Vec<String>);

/// Escritor mínimo de XML: o leiaute não usa atributos além de `versao` e `Id`.
struct Xml(String);

impl Xml {
    fn abrir(&mut self, tag: &str) {
        self.0.push('<');
        self.0.push_str(tag);
        self.0.push('>');
    }

    fn fechar(&mut self, tag: &str) {
        self.0.push_str("</");
        self.0.push_str(tag);
        self.0.push('>');
    }

    fn campo(&mut self, tag: &str, valor: impl AsRef<str>) {
        self.abrir(tag);
        escapar(valor.as_ref(), &mut self.0);
        self.fechar(tag);
    }

    fn opcional<T: AsRef<str>>(&mut self, tag: &str, valor: Option<T>) {
        if let Some(v) = valor {
            self.campo(tag, v);
        }
    }
}

fn escapar(texto: &str, saida: &mut String) {
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            // \r viraria \n no parser; preservar como referência.
            '\r' => saida.push_str("&#xD;"),
            _ => saida.push(c),
        }
    }
}

fn documento(x: &mut Xml, d: &Documento) {
    match d {
        Documento::Cnpj(c) => x.campo("CNPJ", c.as_str()),
        Documento::Cpf(c) => x.campo("CPF", c.as_str()),
    }
}

fn endereco(x: &mut Xml, e: &EnderecoNacional) {
    x.abrir("end");
    x.abrir("endNac");
    x.campo("cMun", &e.municipio);
    x.campo("CEP", &e.cep);
    x.fechar("endNac");
    x.campo("xLgr", &e.logradouro);
    x.campo("nro", &e.numero);
    x.opcional("xCpl", e.complemento.as_ref());
    x.campo("xBairro", &e.bairro);
    x.fechar("end");
}

/// Valida e gera o XML da DPS (sem assinatura). Devolve o XML e o Id de `infDPS`.
pub fn gerar_dps(dps: &Dps) -> Result<(String, DpsId), DpsInvalida> {
    let erros = dps.validar();
    if !erros.is_empty() {
        return Err(DpsInvalida(erros));
    }
    let id = dps.id().map_err(|e| DpsInvalida(vec![e.to_string()]))?;

    let nt009 = dps.leiaute == LeiauteDps::Nt009;
    let mut x = Xml(String::with_capacity(2048));
    x.0.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    x.0.push_str(&format!(
        r#"<DPS xmlns="{NS_NFSE}" versao="{}"><infDPS Id="{}">"#,
        nfse_core::modelo::VERSAO_LEIAUTE,
        id.as_str()
    ));

    x.campo("tpAmb", dps.ambiente.codigo());
    x.campo("dhEmi", dps.emissao.format("%Y-%m-%dT%H:%M:%S%:z").to_string());
    x.campo("verAplic", &dps.versao_aplicativo);
    x.campo("serie", &dps.serie);
    x.campo("nDPS", dps.numero.to_string());
    x.campo("dCompet", dps.competencia.format("%Y-%m-%d").to_string());
    // Emitente: sempre o prestador nesta versão.
    x.campo("tpEmit", "1");
    // NT 009: finNFSe saiu do grupo IBSCBS e fica aqui, antes de cLocEmi (0 = NFS-e regular).
    if nt009 {
        x.campo("finNFSe", "0");
    }
    x.campo("cLocEmi", &dps.municipio_emissor);

    if let Some(s) = &dps.substituicao {
        x.abrir("subst");
        x.campo("chSubstda", &s.chave_substituida);
        x.campo("cMotivo", &s.motivo);
        x.opcional("xMotivo", s.descricao_motivo.as_ref());
        x.fechar("subst");
    }

    let p = &dps.prestador;
    x.abrir("prest");
    documento(&mut x, &p.documento);
    x.opcional("IM", p.inscricao_municipal.as_ref());
    x.opcional("fone", p.telefone.as_ref());
    x.opcional("email", p.email.as_ref());
    x.abrir("regTrib");
    x.campo("opSimpNac", p.regime.simples.codigo());
    x.opcional("regApTribSN", p.regime.apuracao_simples.map(|a| a.codigo()));
    x.campo("regEspTrib", p.regime.regime_especial.to_string());
    x.fechar("regTrib");
    x.fechar("prest");

    if let Some(t) = &dps.tomador {
        x.abrir("toma");
        documento(&mut x, &t.documento);
        x.opcional("IM", t.inscricao_municipal.as_ref());
        x.campo("xNome", &t.nome);
        if let Some(e) = &t.endereco {
            endereco(&mut x, e);
        }
        x.opcional("fone", t.telefone.as_ref());
        x.opcional("email", t.email.as_ref());
        x.fechar("toma");
    }

    let s = &dps.servico;
    x.abrir("serv");
    x.abrir("locPrest");
    x.campo("cLocPrestacao", &s.municipio_prestacao);
    x.fechar("locPrest");
    x.abrir("cServ");
    x.campo("cTribNac", &s.codigo_tributacao_nacional);
    x.opcional("cTribMun", s.codigo_tributacao_municipal.as_ref());
    if nt009 {
        x.opcional("cNBS", s.nbs.as_ref());
        x.campo("xDescServ", &s.descricao);
    } else {
        x.campo("xDescServ", &s.descricao);
        x.opcional("cNBS", s.nbs.as_ref());
    }
    x.fechar("cServ");
    if let Some(o) = &s.obra {
        x.abrir("obra");
        x.opcional("inscImobFisc", o.inscricao_imobiliaria.as_ref());
        match &o.identificacao {
            IdentificacaoObra::Cno(c) => x.campo("cObra", c),
            IdentificacaoObra::Cib(c) => x.campo("cCIB", c),
            IdentificacaoObra::Endereco(e) => {
                x.abrir("end");
                x.campo("CEP", &e.cep);
                x.campo("xLgr", &e.logradouro);
                x.campo("nro", &e.numero);
                x.opcional("xCpl", e.complemento.as_ref());
                x.campo("xBairro", &e.bairro);
                x.fechar("end");
            }
        }
        x.fechar("obra");
    }
    if let Some(info) = &s.informacoes_complementares {
        x.abrir("infoCompl");
        x.campo("xInfComp", info);
        x.fechar("infoCompl");
    }
    x.fechar("serv");

    let v = &dps.valores;
    x.abrir("valores");
    x.abrir("vServPrest");
    x.campo("vServ", v.servico.to_string());
    x.fechar("vServPrest");
    if v.desconto_incondicionado.is_some() || v.desconto_condicionado.is_some() {
        x.abrir("vDescCondIncond");
        x.opcional("vDescIncond", v.desconto_incondicionado.map(|d| d.to_string()));
        x.opcional("vDescCond", v.desconto_condicionado.map(|d| d.to_string()));
        x.fechar("vDescCondIncond");
    }
    x.abrir("trib");
    x.abrir("tribMun");
    x.campo("tribISSQN", v.issqn.tributacao.codigo());
    x.campo("tpRetISSQN", v.issqn.retencao.codigo());
    // E0617: a Sefin calcula a alíquota pelo parâmetro do município quando o prestador não é
    // optante do Simples; informar `pAliq` nesse caso rejeita a DPS.
    if dps.prestador.regime.simples != SituacaoSimples::NaoOptante {
        x.opcional("pAliq", v.issqn.aliquota.map(|a| a.to_string()));
    }
    x.fechar("tribMun");
    if !v.federais.vazio() {
        let f = &v.federais;
        x.abrir("tribFed");
        if let Some(pc) = &f.pis_cofins {
            x.abrir("piscofins");
            x.campo("CST", &pc.cst);
            x.opcional("vBCPisCofins", pc.base_calculo.map(|d| d.to_string()));
            x.opcional("pAliqPis", pc.aliquota_pis.map(|d| d.to_string()));
            x.opcional("pAliqCofins", pc.aliquota_cofins.map(|d| d.to_string()));
            x.opcional("vPis", pc.valor_pis.map(|d| d.to_string()));
            x.opcional("vCofins", pc.valor_cofins.map(|d| d.to_string()));
            x.opcional("tpRetPisCofins", pc.tipo_retencao.map(|t| t.to_string()));
            x.fechar("piscofins");
        }
        x.opcional("vRetCP", f.retencao_cp.map(|d| d.to_string()));
        x.opcional("vRetIRRF", f.retencao_irrf.map(|d| d.to_string()));
        x.opcional("vRetCSLL", f.retencao_csll.map(|d| d.to_string()));
        x.fechar("tribFed");
    }
    x.abrir("totTrib");
    match &v.total_tributos {
        TotalTributos::NaoInformado => x.campo("indTotTrib", "0"),
        TotalTributos::PercentualSimples(p) => x.campo("pTotTribSN", p.to_string()),
        TotalTributos::Percentuais {
            federal,
            estadual,
            municipal,
        } => {
            x.abrir("pTotTrib");
            x.campo("pTotTribFed", federal.to_string());
            x.campo("pTotTribEst", estadual.to_string());
            x.campo("pTotTribMun", municipal.to_string());
            x.fechar("pTotTrib");
        }
        TotalTributos::Valores {
            federal,
            estadual,
            municipal,
        } => {
            x.abrir("vTotTrib");
            x.campo("vTotTribFed", federal.to_string());
            x.campo("vTotTribEst", estadual.to_string());
            x.campo("vTotTribMun", municipal.to_string());
            x.fechar("vTotTrib");
        }
    }
    x.fechar("totTrib");
    x.fechar("trib");
    x.fechar("valores");

    if let Some(i) = &dps.ibs_cbs {
        x.abrir("IBSCBS");
        if !nt009 {
            x.campo("finNFSe", "0");
        }
        x.campo("indFinal", if i.consumo_pessoal == Some(true) { "1" } else { "0" });
        x.campo("cIndOp", &i.indicador_operacao);
        x.opcional("tpOper", i.governamental.map(|g| g.tipo_operacao.to_string()));
        x.opcional("tpEnteGov", i.governamental.map(|g| g.ente.codigo()));
        x.campo("indDest", if i.destinatario_diferente_do_tomador { "1" } else { "0" });
        x.abrir("valores");
        x.abrir("trib");
        if nt009 {
            x.campo("CST", &i.cst);
            x.campo("cClassTrib", &i.classificacao_tributaria);
        } else {
            x.abrir("gIBSCBS");
            x.campo("CST", &i.cst);
            x.campo("cClassTrib", &i.classificacao_tributaria);
            x.fechar("gIBSCBS");
        }
        x.fechar("trib");
        x.fechar("valores");
        x.fechar("IBSCBS");
    }

    x.fechar("infDPS");
    x.fechar("DPS");
    Ok((x.0, id))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn exemplo() -> Dps {
        serde_json::from_str(include_str!("../../../exemplos/dps-exemplo.json")).unwrap()
    }

    #[test]
    fn gera_na_ordem_do_leiaute() {
        let (xml, id) = gerar_dps(&exemplo()).unwrap();
        assert_eq!(id.as_str(), "DPS351110221122233300018100001000000000000001");
        let esperado = concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<DPS xmlns="http://www.sped.fazenda.gov.br/nfse" versao="1.01">"#,
            r#"<infDPS Id="DPS351110221122233300018100001000000000000001">"#,
            "<tpAmb>2</tpAmb><dhEmi>2026-10-16T10:00:00-03:00</dhEmi><verAplic>emissor-nfse/0.1.0</verAplic>",
            "<serie>1</serie><nDPS>1</nDPS><dCompet>2026-10-16</dCompet><tpEmit>1</tpEmit><finNFSe>0</finNFSe><cLocEmi>3511102</cLocEmi>",
            "<prest><CNPJ>11222333000181</CNPJ><IM>12345</IM><fone>1733334444</fone><email>contato@exemplo.com.br</email>",
            "<regTrib><opSimpNac>3</opSimpNac><regApTribSN>1</regApTribSN><regEspTrib>0</regEspTrib></regTrib></prest>",
            "<toma><CNPJ>12ABC34501DE35</CNPJ><xNome>CLINICA EXEMPLO LTDA</xNome>",
            "<end><endNac><cMun>3511102</cMun><CEP>15800000</CEP></endNac><xLgr>RUA EXEMPLO</xLgr><nro>100</nro>",
            "<xBairro>CENTRO</xBairro></end></toma>",
            "<serv><locPrest><cLocPrestacao>3511102</cLocPrestacao></locPrest><cServ><cTribNac>170101</cTribNac><cNBS>113022100</cNBS>",
            "<xDescServ>Serviços prestados referentes a 10/2026 &amp; itens &lt;auxiliares&gt;.</xDescServ>",
            "</cServ></serv>",
            "<valores><vServPrest><vServ>1500.00</vServ></vServPrest><trib><tribMun><tribISSQN>1</tribISSQN>",
            "<tpRetISSQN>1</tpRetISSQN></tribMun><tribFed><piscofins><CST>00</CST><tpRetPisCofins>0</tpRetPisCofins>",
            "</piscofins></tribFed><totTrib><pTotTribSN>11.48</pTotTribSN></totTrib></trib></valores>",
            "</infDPS></DPS>"
        );
        assert_eq!(xml, esperado);
    }

    #[test]
    fn ibs_cbs_leva_ente_governamental_na_ordem_do_leiaute() {
        use nfse_core::modelo::{EnteGovernamental, IbsCbs, OperacaoGovernamental};
        let mut dps = exemplo();
        let mut ibs = IbsCbs {
            consumo_pessoal: None,
            indicador_operacao: "030101".into(),
            destinatario_diferente_do_tomador: false,
            cst: "200".into(),
            classificacao_tributaria: "200029".into(),
            governamental: None,
        };
        dps.ibs_cbs = Some(ibs.clone());
        let (xml, _) = gerar_dps(&dps).unwrap();
        assert!(
            xml.contains("<IBSCBS><indFinal>0</indFinal><cIndOp>030101</cIndOp><indDest>0</indDest>"),
            "{xml}"
        );
        ibs.governamental = Some(OperacaoGovernamental {
            ente: EnteGovernamental::Municipio,
            tipo_operacao: 1,
        });
        dps.ibs_cbs = Some(ibs);
        let (xml, _) = gerar_dps(&dps).unwrap();
        assert!(
            xml.contains("<cIndOp>030101</cIndOp><tpOper>1</tpOper><tpEnteGov>4</tpEnteGov><indDest>0</indDest>"),
            "{xml}"
        );
    }

    #[test]
    fn recusa_dps_invalida() {
        let mut dps = exemplo();
        dps.servico.codigo_tributacao_nacional = "17.19".into();
        let erro = gerar_dps(&dps).unwrap_err();
        assert!(erro.0[0].starts_with("servico.codigo_tributacao_nacional"));
    }

    /// Valida contra os XSDs oficiais versionados em `xsd/` usando o xmllint (libxml2).
    #[test]
    #[ignore = "requer xmllint no PATH"]
    fn valida_no_xsd_oficial() {
        let pfx = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
        let cert = nfse_cert::CertificadoA1::carregar(pfx, "1234").unwrap();
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join("nfse-xml-xsd");
        std::fs::create_dir_all(&dir).unwrap();

        let mut cnpj_numerico = exemplo();
        cnpj_numerico.tomador.as_mut().unwrap().documento =
            Documento::Cnpj(nfse_core::Cnpj::parse("11222333000181").unwrap());
        let mut obra_cno = exemplo();
        obra_cno.servico.codigo_tributacao_nacional = "070201".into();
        obra_cno.servico.obra = Some(nfse_core::modelo::Obra {
            inscricao_imobiliaria: Some("01.02.003.0004".into()),
            identificacao: IdentificacaoObra::Cno("900112345678".into()),
        });
        let mut obra_endereco = obra_cno.clone();
        obra_endereco.servico.obra = Some(nfse_core::modelo::Obra {
            inscricao_imobiliaria: None,
            identificacao: IdentificacaoObra::Endereco(nfse_core::modelo::EnderecoObra {
                cep: "15800000".into(),
                logradouro: "RUA DA OBRA".into(),
                numero: "10".into(),
                complemento: None,
                bairro: "CENTRO".into(),
            }),
        });
        let mut ente_publico = exemplo();
        ente_publico.ibs_cbs = Some(nfse_core::modelo::IbsCbs {
            consumo_pessoal: None,
            indicador_operacao: "030101".into(),
            destinatario_diferente_do_tomador: false,
            cst: "200".into(),
            classificacao_tributaria: "200029".into(),
            governamental: Some(nfse_core::modelo::OperacaoGovernamental {
                ente: nfse_core::modelo::EnteGovernamental::Municipio,
                tipo_operacao: 1,
            }),
        });
        let casos = [exemplo(), cnpj_numerico, obra_cno, obra_endereco, ente_publico];
        // Só pacotes de Produção Restrita: o de Produção (09/02/2026) tem o padrão da série com
        // `^...$`, que em XSD são literais, e recusa qualquer série.
        for (leiaute, versao) in [
            (LeiauteDps::Nt009, "v1.01-prodrest-20261020"),
            (LeiauteDps::Nt004, "v1.01-prodrest-20260727"),
        ] {
            for (i, dps) in casos.iter().cloned().enumerate() {
                let mut dps = dps;
                dps.leiaute = leiaute;
                let (xml, id) = gerar_dps(&dps).unwrap();
                for (sufixo, conteudo) in [
                    ("", xml.clone()),
                    ("-assinada", crate::assinar(&xml, id.as_str(), &cert).unwrap()),
                ] {
                    let arquivo = dir.join(format!("dps-{i}{sufixo}.xml"));
                    std::fs::write(&arquivo, conteudo).unwrap();
                    let saida = std::process::Command::new("xmllint")
                        .arg("--noout")
                        .arg("--schema")
                        .arg(raiz.join("xsd").join(versao).join("DPS_v1.01.xsd"))
                        .arg(&arquivo)
                        .output()
                        .unwrap();
                    assert!(
                        saida.status.success(),
                        "{versao} {}: {}",
                        arquivo.display(),
                        String::from_utf8_lossy(&saida.stderr)
                    );
                }
            }
        }
    }

    #[test]
    fn leiaute_nt004_mantem_a_ordem_anterior() {
        use nfse_core::modelo::IbsCbs;
        let mut dps = exemplo();
        dps.leiaute = LeiauteDps::Nt004;
        dps.ibs_cbs = Some(IbsCbs {
            consumo_pessoal: None,
            indicador_operacao: "030101".into(),
            destinatario_diferente_do_tomador: false,
            cst: "200".into(),
            classificacao_tributaria: "200029".into(),
            governamental: None,
        });
        let (xml, _) = gerar_dps(&dps).unwrap();
        assert!(xml.contains("<tpEmit>1</tpEmit><cLocEmi>"), "{xml}");
        assert!(xml.contains("<IBSCBS><finNFSe>0</finNFSe><indFinal>"), "{xml}");
        assert!(xml.contains("</xDescServ><cNBS>"), "{xml}");
        assert!(
            xml.contains("<trib><gIBSCBS><CST>200</CST><cClassTrib>200029</cClassTrib></gIBSCBS></trib>"),
            "{xml}"
        );
    }

    #[test]
    fn gera_e_assina() {
        let pfx = include_bytes!("../../nfse-cert/tests/fixtures/teste-ecnpj-aes.pfx");
        let cert = nfse_cert::CertificadoA1::carregar(pfx, "1234").unwrap();
        let (xml, id) = gerar_dps(&exemplo()).unwrap();
        let assinado = crate::assinar(&xml, id.as_str(), &cert).unwrap();
        crate::verificar(&assinado).unwrap();
    }
}
