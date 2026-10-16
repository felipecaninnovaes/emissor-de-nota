//! Leitura dos dados da NFS-e devolvida pela Sefin (para gravar no banco e montar a pasta/PDF).

use crate::dps::NS_NFSE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosNfse {
    /// Chave de acesso (50 posições), tirada do `Id` de `infNFSe` sem o prefixo "NFS".
    pub chave: String,
    /// `nNFSe`
    pub numero: String,
    /// `cStat` (100 = autorizada)
    pub situacao: Option<String>,
    /// `infDPS/@Id` da DPS encapsulada.
    pub id_dps: Option<String>,
    /// `dCompet` da DPS (AAAA-MM-DD)
    pub competencia: Option<String>,
    /// `vServ` da DPS
    pub valor_servico: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErroLeituraNfse {
    #[error("XML da NFS-e malformado: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("XML não é uma NFS-e do padrão nacional (falta {0})")]
    Estrutura(&'static str),
}

pub fn ler_nfse(xml: &str) -> Result<DadosNfse, ErroLeituraNfse> {
    let doc = roxmltree::Document::parse(xml)?;
    let elemento = |nome: &str| {
        doc.descendants()
            .find(|n| n.tag_name().namespace() == Some(NS_NFSE) && n.tag_name().name() == nome)
    };
    let texto = |nome: &str| elemento(nome).and_then(|n| n.text()).map(str::to_owned);

    let inf = elemento("infNFSe").ok_or(ErroLeituraNfse::Estrutura("infNFSe"))?;
    let chave = inf
        .attribute("Id")
        .and_then(|id| id.strip_prefix("NFS"))
        .ok_or(ErroLeituraNfse::Estrutura("infNFSe/@Id"))?
        .to_owned();
    Ok(DadosNfse {
        chave,
        numero: texto("nNFSe").ok_or(ErroLeituraNfse::Estrutura("nNFSe"))?,
        situacao: texto("cStat"),
        id_dps: elemento("infDPS").and_then(|n| n.attribute("Id")).map(str::to_owned),
        competencia: texto("dCompet"),
        valor_servico: texto("vServ"),
    })
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;

    /// NFS-e fictícia no formato devolvido pela Sefin (sem assinatura).
    pub fn nfse_exemplo(chave: &str, id_dps: &str) -> String {
        format!(
            concat!(
                r#"<?xml version="1.0" encoding="utf-8"?><NFSe versao="1.01" xmlns="http://www.sped.fazenda.gov.br/nfse">"#,
                r#"<infNFSe Id="NFS{chave}"><nNFSe>77</nNFSe><cStat>100</cStat>"#,
                r#"<DPS versao="1.01" xmlns="http://www.sped.fazenda.gov.br/nfse"><infDPS Id="{id_dps}">"#,
                "<dCompet>2026-10-16</dCompet><valores><vServPrest><vServ>1.00</vServ></vServPrest></valores>",
                "</infDPS></DPS></infNFSe></NFSe>"
            ),
            chave = chave,
            id_dps = id_dps
        )
    }

    #[test]
    fn le_dados() {
        let xml = nfse_exemplo("35111022211222333000181000000000007726107413634280", "DPS1");
        let d = ler_nfse(&xml).unwrap();
        assert_eq!(d.chave, "35111022211222333000181000000000007726107413634280");
        assert_eq!(d.numero, "77");
        assert_eq!(d.situacao.as_deref(), Some("100"));
        assert_eq!(d.id_dps.as_deref(), Some("DPS1"));
        assert_eq!(d.competencia.as_deref(), Some("2026-10-16"));
        assert_eq!(d.valor_servico.as_deref(), Some("1.00"));
    }

    #[test]
    fn recusa_outro_documento() {
        assert!(matches!(ler_nfse("<DPS/>"), Err(ErroLeituraNfse::Estrutura("infNFSe"))));
    }
}
