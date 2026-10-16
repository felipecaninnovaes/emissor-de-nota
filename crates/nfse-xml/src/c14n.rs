//! Canonical XML 1.0 (<http://www.w3.org/TR/2001/REC-xml-c14n-20010315>), inclusivo, e
//! Exclusive XML Canonicalization (<http://www.w3.org/2001/10/xml-exc-c14n#>), com ou sem comentários.
//!
//! A DPS e os eventos que assinamos usam o inclusivo sem comentários; a NFS-e devolvida pela Sefin
//! vem assinada com o exclusivo. Cobre subárvore de um elemento (com os namespaces herdados dos
//! ancestrais), ordenação de namespaces e atributos, escape de texto e atributos e exclusão de um
//! nó (transformação enveloped-signature). Não trata DTD, atributos `xml:*` herdados nem a lista
//! `InclusiveNamespaces`, que não aparecem no leiaute nacional.

use roxmltree::{Node, NodeId, NodeType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Opcoes {
    /// Exclusivo: declara só os namespaces usados pelo próprio elemento e seus atributos.
    pub exclusivo: bool,
    pub comentarios: bool,
}

/// C14N inclusivo sem comentários de `no` e descendentes. `excluir` é omitido junto com a subárvore.
pub fn canonicalizar(no: Node, excluir: Option<NodeId>) -> String {
    canonicalizar_com(no, excluir, Opcoes::default())
}

pub fn canonicalizar_com(no: Node, excluir: Option<NodeId>, opcoes: Opcoes) -> String {
    let mut saida = String::new();
    escrever(no, &[], excluir, opcoes, &mut saida);
    saida
}

/// Namespaces já declarados na saída pelos ancestrais: (prefixo, URI); prefixo "" = padrão.
type Escopo<'a> = Vec<(&'a str, &'a str)>;

fn escopo<'a>(no: Node<'a, '_>) -> Escopo<'a> {
    no.namespaces()
        .map(|ns| (ns.name().unwrap_or(""), ns.uri()))
        .filter(|(prefixo, _)| *prefixo != "xml")
        .collect()
}

fn uri_renderizada<'a>(renderizados: &[(&'a str, &'a str)], prefixo: &str) -> &'a str {
    renderizados
        .iter()
        .rev()
        .find(|(p, _)| *p == prefixo)
        .map_or("", |(_, u)| u)
}

fn escrever<'a>(
    no: Node<'a, '_>,
    renderizados: &[(&'a str, &'a str)],
    excluir: Option<NodeId>,
    opcoes: Opcoes,
    saida: &mut String,
) {
    if Some(no.id()) == excluir {
        return;
    }
    match no.node_type() {
        NodeType::Element => escrever_elemento(no, renderizados, excluir, opcoes, saida),
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
        NodeType::Comment if opcoes.comentarios => {
            saida.push_str("<!--");
            saida.push_str(no.text().unwrap_or(""));
            saida.push_str("-->");
        }
        NodeType::Comment | NodeType::Root => {}
    }
}

/// Prefixo usado para `uri` no elemento (`""` = namespace padrão). Atributos nunca usam o padrão.
fn prefixo<'a>(no: Node<'a, '_>, uri: Option<&str>, eh_atributo: bool) -> &'a str {
    let Some(uri) = uri else { return "" };
    no.namespaces()
        .filter(|ns| !(eh_atributo && ns.name().is_none()))
        .find(|ns| ns.uri() == uri)
        .and_then(|ns| ns.name())
        .unwrap_or("")
}

fn qualificar(prefixo: &str, local: &str) -> String {
    if prefixo.is_empty() {
        local.to_owned()
    } else {
        format!("{prefixo}:{local}")
    }
}

fn declarar(prefixo: &str, uri: &str, saida: &mut String) {
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

fn escrever_elemento<'a>(
    no: Node<'a, '_>,
    renderizados: &[(&'a str, &'a str)],
    excluir: Option<NodeId>,
    opcoes: Opcoes,
    saida: &mut String,
) {
    let tag = no.tag_name();
    let nome = qualificar(prefixo(no, tag.namespace(), false), tag.name());
    saida.push('<');
    saida.push_str(&nome);

    let em_escopo = escopo(no);
    let uri_em_escopo = |p: &str| em_escopo.iter().find(|(q, _)| *q == p).map_or("", |(_, u)| *u);
    let mut atributos: Vec<_> = no.attributes().collect();

    // Candidatos: inclusivo = todos em escopo; exclusivo = só os visivelmente usados.
    let mut candidatos: Escopo<'a> = if opcoes.exclusivo {
        let mut usados = vec![prefixo(no, tag.namespace(), false)];
        usados.extend(
            atributos
                .iter()
                .map(|a| prefixo(no, a.namespace(), true))
                .filter(|p| !p.is_empty()),
        );
        usados.sort();
        usados.dedup();
        usados.into_iter().map(|p| (p, uri_em_escopo(p))).collect()
    } else {
        let mut todos = em_escopo.clone();
        // Padrão ausente em escopo conta como "" para poder desfazer o do ancestral (xmlns="").
        if !todos.iter().any(|(p, _)| p.is_empty()) {
            todos.push(("", ""));
        }
        todos
    };
    candidatos.sort();

    // Declara só o que difere do que os ancestrais de saída já declararam, padrão primeiro.
    let mut proximos: Escopo<'a> = renderizados.to_vec();
    for (p, uri) in candidatos {
        if uri_renderizada(renderizados, p) == uri {
            continue;
        }
        declarar(p, uri, saida);
        proximos.push((p, uri));
    }

    // Atributos ordenados por (URI do namespace, nome local); sem namespace vem primeiro.
    atributos.sort_by(|a, b| (a.namespace().unwrap_or(""), a.name()).cmp(&(b.namespace().unwrap_or(""), b.name())));
    for attr in atributos {
        saida.push(' ');
        saida.push_str(&qualificar(prefixo(no, attr.namespace(), true), attr.name()));
        saida.push_str("=\"");
        escapar_atributo(attr.value(), saida);
        saida.push('"');
    }
    saida.push('>');

    for filho in no.children() {
        escrever(filho, &proximos, excluir, opcoes, saida);
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

    fn c14n_com(xml: &str, nome: &str, opcoes: Opcoes) -> String {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let no = doc.descendants().find(|n| n.has_tag_name(nome)).unwrap();
        canonicalizar_com(no, None, opcoes)
    }

    #[test]
    fn exclusivo_declara_so_o_usado() {
        let xml = r#"<r xmlns="urn:d" xmlns:a="urn:a" xmlns:b="urn:b"><e a:x="1"><f/><b:g/></e></r>"#;
        let exc = Opcoes {
            exclusivo: true,
            comentarios: false,
        };
        assert_eq!(
            c14n_com(xml, "e", exc),
            r#"<e xmlns="urn:d" xmlns:a="urn:a" a:x="1"><f></f><b:g xmlns:b="urn:b"></b:g></e>"#
        );
        // O inclusivo leva todos os namespaces em escopo para o ápice.
        assert_eq!(
            c14n_com(xml, "e", Opcoes::default()),
            r#"<e xmlns="urn:d" xmlns:a="urn:a" xmlns:b="urn:b" a:x="1"><f></f><b:g></b:g></e>"#
        );
    }

    #[test]
    fn comentarios_opcionais() {
        let xml = "<r><!-- nota -->t</r>";
        assert_eq!(c14n_com(xml, "r", Opcoes::default()), "<r>t</r>");
        assert_eq!(
            c14n_com(
                xml,
                "r",
                Opcoes {
                    exclusivo: true,
                    comentarios: true
                }
            ),
            "<r><!-- nota -->t</r>"
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
