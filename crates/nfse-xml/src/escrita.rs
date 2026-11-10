//! Escritor de XML dos documentos que o emissor gera (DPS e pedido de evento): um só escape para
//! texto. A canonicalização (`c14n.rs`) tem os escapes próprios da especificação, que são outros.

/// Escritor mínimo de XML: os leiautes não usam atributos além de `xmlns`, `versao` e `Id`, que a
/// raiz escreve com [`Xml::raiz`].
pub(crate) struct Xml(pub(crate) String);

impl Xml {
    /// Declaração XML e as tags de abertura da raiz e do grupo assinado (`Id`).
    pub(crate) fn raiz(raiz: &str, grupo: &str, id: &str, capacidade: usize) -> Self {
        let mut x = Xml(String::with_capacity(capacidade));
        x.0.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        x.0.push_str(&format!(
            r#"<{raiz} xmlns="{}" versao="{}"><{grupo} Id="{id}">"#,
            crate::dps::NS_NFSE,
            nfse_core::modelo::VERSAO_LEIAUTE,
        ));
        x
    }

    pub(crate) fn abrir(&mut self, tag: &str) {
        self.0.push('<');
        self.0.push_str(tag);
        self.0.push('>');
    }

    pub(crate) fn fechar(&mut self, tag: &str) {
        self.0.push_str("</");
        self.0.push_str(tag);
        self.0.push('>');
    }

    pub(crate) fn campo(&mut self, tag: &str, valor: impl AsRef<str>) {
        self.abrir(tag);
        escapar(valor.as_ref(), &mut self.0);
        self.fechar(tag);
    }

    pub(crate) fn opcional<T: AsRef<str>>(&mut self, tag: &str, valor: Option<T>) {
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
