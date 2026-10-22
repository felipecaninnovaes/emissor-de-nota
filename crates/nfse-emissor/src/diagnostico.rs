//! Texto de diagnóstico de uma DPS para colar num chamado ou numa conversa de suporte: situação, erro,
//! resposta da Sefin e o XML enviado, com documentos, nomes, endereço e descrição do serviço mascarados.
//! Valores, alíquotas, códigos e a estrutura do XML ficam, que é o que ajuda a achar o problema.

use nfse_db::dps::SituacaoDps;

use super::{Emissor, ErroEmissao};
use crate::Conector;

/// Elementos da DPS cujo conteúdo identifica pessoas ou descreve o serviço.
const ELEMENTOS_MASCARADOS: &[&str] = &[
    "CNPJ",
    "CPF",
    "NIF",
    "IM",
    "xNome",
    "xLgr",
    "nro",
    "xCpl",
    "xBairro",
    "CEP",
    "fone",
    "email",
    "xDescServ",
    "xInfComp",
    "xLocPrestacao",
    "xPais",
];

const OMITIDO: &str = "***";

/// Troca o conteúdo de `<tag>…</tag>` por `***`, em todas as ocorrências.
fn mascarar_elemento(xml: &str, tag: &str) -> String {
    let abre = format!("<{tag}>");
    let fecha = format!("</{tag}>");
    let mut saida = String::with_capacity(xml.len());
    let mut resto = xml;
    while let Some(i) = resto.find(&abre) {
        let depois = i + abre.len();
        let Some(f) = resto[depois..].find(&fecha) else { break };
        saida.push_str(&resto[..depois]);
        saida.push_str(OMITIDO);
        resto = &resto[depois + f..];
    }
    saida.push_str(resto);
    saida
}

/// Esconde o documento embutido no Id da DPS (`DPS` + município 7 + tipo 1 + documento 14 + série 5 +
/// número 15): a série e o número, que ajudam no suporte, continuam visíveis.
fn mascarar_ids(texto: &str) -> String {
    const DIGITOS: usize = 42;
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find("DPS") {
        let inicio = i + 3;
        saida.push_str(&resto[..inicio]);
        let candidato = &resto[inicio..];
        let digitos = candidato.bytes().take_while(u8::is_ascii_digit).count();
        if digitos == DIGITOS {
            saida.push_str(&candidato[..8]);
            saida.push_str(&"*".repeat(14));
            saida.push_str(&candidato[22..DIGITOS]);
            resto = &candidato[DIGITOS..];
        } else {
            resto = candidato;
        }
    }
    saida.push_str(resto);
    saida
}

/// Remove a assinatura (só ruído para o diagnóstico) e mascara os dados pessoais do XML.
pub fn mascarar_xml(xml: &str) -> String {
    let mut saida = mascarar_ids(xml);
    if let (Some(i), Some(f)) = (saida.find("<Signature"), saida.find("</Signature>"))
        && f > i
    {
        saida.replace_range(i..f + "</Signature>".len(), "<Signature>…</Signature>");
    }
    for tag in ELEMENTOS_MASCARADOS {
        saida = mascarar_elemento(&saida, tag);
    }
    saida
}

impl<C: Conector> Emissor<C> {
    /// Diagnóstico de uma DPS pronto para copiar, com os dados pessoais mascarados.
    pub async fn diagnostico_nota(&self, dps_id: i64) -> Result<String, ErroEmissao> {
        let dps = self.banco.dps(dps_id).await?;
        let modelo = self.banco.modelo_dps(dps_id).await?;
        let resposta = self.banco.resposta_sefin(dps_id).await?;
        let ambiente = match dps.ambiente {
            nfse_core::modelo::Ambiente::Producao => "Produção",
            nfse_core::modelo::Ambiente::Homologacao => "Produção Restrita",
        };
        let mut t = String::new();
        t.push_str(&format!("Diagnóstico da DPS {}/{}\n", dps.serie, dps.numero));
        t.push_str(&format!("Aplicativo: {}\n", self.versao_aplicativo));
        t.push_str(&format!("Ambiente: {ambiente}\n"));
        t.push_str(&format!(
            "Leiaute da DPS: {}\n",
            modelo.as_ref().map_or("?", |m| m.leiaute.codigo())
        ));
        t.push_str(&format!("Id: {}\n", mascarar_ids(&dps.id_dps)));
        t.push_str(&format!("Situação: {}\n", dps.situacao.texto()));
        t.push_str(&format!("Tentativas: {}\n", dps.tentativas));
        if let Some(enviado) = &dps.enviado_em {
            t.push_str(&format!("Último envio (UTC): {enviado}\n"));
        }
        if let Some(erro) = &dps.ultimo_erro {
            t.push_str(&format!("Erro: {}\n", mascarar_ids(erro)));
        }
        if dps.situacao == SituacaoDps::Reservada {
            t.push_str("\n(DPS reservada, ainda não enviada)\n");
        }
        if let Some(r) = resposta {
            t.push_str(&format!("\n--- Resposta da Sefin ---\n{}\n", mascarar_ids(&r)));
        }
        if let Some(xml) = &dps.xml_assinado {
            t.push_str(&format!(
                "\n--- DPS enviada (documentos, nomes, endereço e descrição omitidos) ---\n{}\n",
                mascarar_xml(xml)
            ));
        }
        Ok(t)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const ID: &str = "DPS351110225184338100010100001000000000000001";

    #[test]
    fn mascara_documentos_nomes_e_descricao_mas_mantem_valores() {
        let xml = format!(
            r#"<DPS><infDPS Id="{ID}"><serie>1</serie><prest><CNPJ>51843381000101</CNPJ><IM>11243</IM></prest><toma><CNPJ>45118429001279</CNPJ><xNome>FULANO DE TAL</xNome><end><xLgr>RUA A</xLgr><nro>255</nro></end></toma><serv><cServ><cTribNac>040101</cTribNac><xDescServ>Plantão do Dr. Fulano</xDescServ></cServ></serv><valores><vServ>760.00</vServ></valores></infDPS><Signature xmlns="x"><SignedInfo>abc</SignedInfo></Signature></DPS>"#
        );
        let m = mascarar_xml(&xml);
        for segredo in [
            "51843381000101",
            "45118429001279",
            "FULANO",
            "RUA A",
            "11243",
            "Plantão",
        ] {
            assert!(!m.contains(segredo), "{segredo} vazou: {m}");
        }
        assert!(m.contains("<CNPJ>***</CNPJ>") && m.contains("<xNome>***</xNome>"));
        assert!(m.contains("<vServ>760.00</vServ>") && m.contains("<cTribNac>040101</cTribNac>"));
        assert!(m.contains("<Signature>…</Signature>") && !m.contains("SignedInfo"));
        // série e número do Id continuam visíveis
        assert!(m.contains("DPS35111022**************00001000000000000001"), "{m}");
    }

    #[test]
    fn mascara_o_documento_do_id_da_dps() {
        let m = mascarar_ids(&format!("idDPS {ID} e DPS123"));
        assert!(!m.contains("51843381000101"), "{m}");
        assert!(m.contains("00001000000000000001"));
        assert!(m.ends_with("DPS123"));
    }

    #[test]
    fn xml_sem_fechamento_nao_trava() {
        assert_eq!(mascarar_elemento("<CNPJ>123", "CNPJ"), "<CNPJ>123");
    }
}
