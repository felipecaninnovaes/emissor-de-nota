//! Canonical XML 1.0 inclusivo (<http://www.w3.org/TR/2001/REC-xml-c14n-20010315>), sem comentários.
//!
//! Cobre o que a DPS e os eventos usam: subárvore de um elemento (com os namespaces herdados
//! dos ancestrais), ordenação de namespaces e atributos, escape de texto e atributos e
//! exclusão de um nó (transformação enveloped-signature). Não trata DTD nem atributos
//! `xml:*` herdados, que não aparecem no leiaute nacional.

use roxmltree::{Node, NodeId, NodeType};

/// Canoniza `no` e seus descendentes. `excluir`, se informado, é omitido junto com a subárvore.
pub fn canonicalizar(no: Node, excluir: Option<NodeId>) -> String {
    let mut saida = String::new();
    escrever(no, &[], excluir, &mut saida);
    saida
}

/// Namespaces já declarados pelo ancestral de saída mais próximo: (prefixo, URI); prefixo "" = padrão.
type Escopo<'a> = Vec<(&'a str, &'a str)>;

fn escopo<'a>(no: Node<'a, '_>) -> Escopo<'a> {
    no.namespaces()
        .map(|ns| (ns.name().unwrap_or(""), ns.uri()))
        .filter(|(prefixo, _)| *prefixo != "xml")
        .collect()
}

fn escrever<'a>(no: Node<'a, '_>, renderizados: &[(&'a str, &'a str)], excluir: Option<NodeId>, saida: &mut String) {
    if Some(no.id()) == excluir {
        return;
    }
    match no.node_type() {
        NodeType::Element => escrever_elemento(no, renderizados, excluir, saida),
        NodeType::Text => escapar_texto(no.text().unwrap_or(""), saida),
        NodeType::PI => {
            if let Some(pi) = no.pi() {
                saida.push_str("<?");
                saida.push_str(pi.target);
                if let Some(valor) = pi.value {
                    saida.push(' ');
                    saida.push_str(valor);
                }
                saida.push_str("?>");
            }
        }
        NodeType::Comment | NodeType::Root => {}
    }
}

fn nome_qualificado(no: Node, uri: Option<&str>, local: &str, eh_atributo: bool) -> String {
    let Some(uri) = uri else { return local.to_owned() };
    let prefixo = no
        .namespaces()
        .filter(|ns| !(eh_atributo && ns.name().is_none()))
        .find(|ns| ns.uri() == uri)
        .and_then(|ns| ns.name());
    match prefixo {
        Some(p) => format!("{p}:{local}"),
        None => local.to_owned(),
    }
}

fn escrever_elemento<'a>(
    no: Node<'a, '_>,
    renderizados: &[(&'a str, &'a str)],
    excluir: Option<NodeId>,
    saida: &mut String,
) {
    let tag = no.tag_name();
    let nome = nome_qualificado(no, tag.namespace(), tag.name(), false);
    saida.push('<');
    saida.push_str(&nome);

    // Namespaces: só os que mudaram em relação ao ancestral de saída, padrão primeiro e depois por prefixo.
    let mut atual = escopo(no);
    atual.sort();
    for &(prefixo, uri) in &atual {
        if renderizados.contains(&(prefixo, uri)) {
            continue;
        }
        if prefixo.is_empty() {
            saida.push_str(" xmlns=\"");
        } else {
            saida.push_str(" xmlns:");
            saida.push_str(prefixo);
            saida.push_str("=\"");
        }
        escapar_atributo(uri, saida);
        saida.push('"');
    }
    // Namespace padrão desfeito (xmlns="") quando o ancestral de saída tinha um padrão não vazio.
    let padrao_ancestral = renderizados.iter().any(|(p, u)| p.is_empty() && !u.is_empty());
    if padrao_ancestral && !atual.iter().any(|(p, _)| p.is_empty()) {
        saida.push_str(" xmlns=\"\"");
    }

    // Atributos ordenados por (URI do namespace, nome local); sem namespace vem primeiro.
    let mut atributos: Vec<_> = no.attributes().collect();
    atributos.sort_by(|a, b| (a.namespace().unwrap_or(""), a.name()).cmp(&(b.namespace().unwrap_or(""), b.name())));
    for attr in atributos {
        saida.push(' ');
        saida.push_str(&nome_qualificado(no, attr.namespace(), attr.name(), true));
        saida.push_str("=\"");
        escapar_atributo(attr.value(), saida);
        saida.push('"');
    }
    saida.push('>');

    for filho in no.children() {
        escrever(filho, &atual, excluir, saida);
    }

    saida.push_str("</");
    saida.push_str(&nome);
    saida.push('>');
}

fn escapar_texto(texto: &str, saida: &mut String) {
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '\r' => saida.push_str("&#xD;"),
            _ => saida.push(c),
        }
    }
}

fn escapar_atributo(valor: &str, saida: &mut String) {
    for c in valor.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '"' => saida.push_str("&quot;"),
            '\t' => saida.push_str("&#x9;"),
            '\n' => saida.push_str("&#xA;"),
            '\r' => saida.push_str("&#xD;"),
            _ => saida.push(c),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c14n_do_elemento(xml: &str, nome: &str) -> String {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let no = doc.descendants().find(|n| n.has_tag_name(nome)).unwrap();
        canonicalizar(no, None)
    }

    #[test]
    fn herda_namespace_do_ancestral() {
        let xml =
            r#"<DPS xmlns="http://www.sped.fazenda.gov.br/nfse" versao="1.01"><infDPS Id="X"><a/></infDPS></DPS>"#;
        assert_eq!(
            c14n_do_elemento(xml, "infDPS"),
            r#"<infDPS xmlns="http://www.sped.fazenda.gov.br/nfse" Id="X"><a></a></infDPS>"#
        );
    }

    #[test]
    fn ordena_atributos_e_namespaces() {
        let xml = r#"<r xmlns:b="urn:b" xmlns:a="urn:a" z="1" b:y="2" a:y="3" m="4"/>"#;
        assert_eq!(
            c14n_do_elemento(xml, "r"),
            r#"<r xmlns:a="urn:a" xmlns:b="urn:b" m="4" z="1" a:y="3" b:y="2"></r>"#
        );
    }

    #[test]
    fn escapes_e_entidades() {
        let xml = "<?xml version=\"1.0\"?>\n<r a='x&amp;&quot;&lt;&#9;'>A &amp; B &lt; C &gt; D &#233;<!-- fora --><![CDATA[<cdata>]]></r>";
        assert_eq!(
            c14n_do_elemento(xml, "r"),
            "<r a=\"x&amp;&quot;&lt;&#x9;\">A &amp; B &lt; C &gt; D é&lt;cdata&gt;</r>"
        );
    }

    #[test]
    fn nao_repete_namespace_e_preserva_espacos() {
        let xml = "<a xmlns=\"urn:x\">\n  <b xmlns=\"urn:x\">t</b>\n  <c xmlns=\"\"/>\n</a>";
        assert_eq!(
            c14n_do_elemento(xml, "a"),
            "<a xmlns=\"urn:x\">\n  <b>t</b>\n  <c xmlns=\"\"></c>\n</a>"
        );
    }
}
