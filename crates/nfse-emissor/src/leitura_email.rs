//! Leitura do corpo de um e-mail salvo (`.eml`) em texto, para a importação de pedidos de nota.
//!
//! Do `.eml` (MIME) vale o primeiro corpo em texto puro; sem ele, o HTML convertido em texto. A
//! decodificação (quoted-printable, base64 e charset) é do `mail-parser`. Os anexos são ignorados.
//! O texto lido nunca vai para log nem para mensagem de erro (dados de pacientes).

use mail_parser::{HeaderName, MessageParser, PartType};

/// Erro de arquivo que não é um e-mail legível.
pub const ERRO_ABRIR: &str = "não consegui abrir o e-mail";
/// Erro de e-mail sem corpo em texto ou HTML.
pub const ERRO_CORPO: &str = "não consegui ler o corpo deste e-mail; cole o texto";

/// Texto do corpo do e-mail, pelo nome do arquivo (extensão) e conteúdo.
pub fn texto_do_email(nome_arquivo: &str, bytes: &[u8]) -> Result<String, String> {
    let extensao = nome_arquivo
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match extensao.as_str() {
        "eml" => ler_eml(bytes),
        _ => Err("use um e-mail .eml ou .msg".into()),
    }
}

/// Se o arquivo é um e-mail pela extensão (`.eml` ou `.msg`).
pub fn e_email(nome_arquivo: &str) -> bool {
    nome_arquivo
        .rsplit_once('.')
        .is_some_and(|(_, e)| e.eq_ignore_ascii_case("eml") || e.eq_ignore_ascii_case("msg"))
}

fn ler_eml(bytes: &[u8]) -> Result<String, String> {
    let mensagem = MessageParser::default()
        .parse(bytes)
        // Lixo binário também "tem cabeçalhos" para o parser; exige ao menos um conhecido.
        .filter(|m| m.headers().iter().any(|h| !matches!(h.name, HeaderName::Other(_))))
        .ok_or_else(|| ERRO_ABRIR.to_string())?;
    // `text_body` aponta para o texto puro ou, sem ele, para o HTML.
    let parte = mensagem
        .text_body
        .first()
        .and_then(|&i| mensagem.parts.get(i as usize))
        .ok_or_else(|| ERRO_CORPO.to_string())?;
    let texto = match &parte.body {
        PartType::Text(t) => arrumar_linhas(t),
        PartType::Html(h) => html_em_texto(h),
        _ => return Err(ERRO_CORPO.into()),
    };
    if texto.trim().is_empty() {
        return Err(ERRO_CORPO.into());
    }
    Ok(texto)
}

/// HTML em texto, com uma linha por parágrafo, `<div>`, linha de tabela ou item de lista.
///
/// O conversor do `mail-parser` só quebra a linha em `<br>` e `</p>`; o Outlook e os webmails
/// usam `<div>` e tabelas, então os fechamentos desses blocos viram `<br>` antes.
pub(crate) fn html_em_texto(html: &str) -> String {
    const BLOCOS: [&str; 5] = ["</div", "</tr", "</li", "</h", "</table"];
    let minusculo = html.to_ascii_lowercase();
    let mut com_quebras = String::with_capacity(html.len() + 64);
    let mut inicio = 0;
    let mut i = 0;
    while let Some(pos) = minusculo[i..].find("</").map(|p| p + i) {
        if BLOCOS.iter().any(|b| minusculo[pos..].starts_with(b)) {
            com_quebras.push_str(&html[inicio..pos]);
            com_quebras.push_str("<br>");
            inicio = pos;
        }
        i = pos + 2;
    }
    com_quebras.push_str(&html[inicio..]);
    arrumar_linhas(&mail_parser::decoders::html::html_to_text(&com_quebras))
}

/// Troca espaço não separável por espaço, tira espaços no fim das linhas e junta linhas vazias.
fn arrumar_linhas(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut vazias = 0;
    for linha in texto.replace('\u{a0}', " ").lines() {
        let linha = linha.trim_end();
        if linha.trim().is_empty() {
            vazias += 1;
            if vazias > 1 {
                continue;
            }
        } else {
            vazias = 0;
        }
        saida.push_str(linha);
        saida.push('\n');
    }
    saida.trim_matches('\n').to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn eml(cabecalhos: &str, corpo: &str) -> Vec<u8> {
        format!(
            "From: Clinica Ficticia <pedidos@exemplo.com.br>\r\nTo: escritorio@exemplo.com.br\r\n\
             Subject: Notas de outubro\r\nMIME-Version: 1.0\r\n{cabecalhos}\r\n\r\n{corpo}"
        )
        .into_bytes()
    }

    #[test]
    fn eml_quoted_printable_iso_8859_1() {
        let bytes = eml(
            "Content-Type: text/plain; charset=ISO-8859-1\r\nContent-Transfer-Encoding: quoted-printable",
            "PACIENTE: Jos=E9 da Concei=E7=E3o\r\nENDERE=C7O: Rua das Ac=E1cias, 10\r\nVALOR: R$ 150,00\r\n",
        );
        let texto = texto_do_email("pedido.eml", &bytes).unwrap();
        assert!(texto.contains("PACIENTE: José da Conceição"), "{texto}");
        assert!(texto.contains("ENDEREÇO: Rua das Acácias, 10"), "{texto}");
        assert!(texto.contains("VALOR: R$ 150,00"), "{texto}");
    }

    #[test]
    fn eml_so_html_base64() {
        let bytes = eml(
            "Content-Type: text/html; charset=UTF-8\r\nContent-Transfer-Encoding: base64",
            "PGh0bWw+PGhlYWQ+PHN0eWxlPnB7bWFyZ2luOjB9PC9zdHlsZT48L2hlYWQ+PGJvZHk+PHAgY2xh\r\n\
             c3M9Ik1zb05vcm1hbCI+Qm9tIGRpYSw8L3A+PGRpdj5QQUNJRU5URTogTWFyaWEgQ29uY2VpJmNj\r\n\
             ZWRpbDsmYXRpbGRlO288L2Rpdj48ZGl2PkNQRjogNTI5Ljk4Mi4yNDctMjU8L2Rpdj48cD5WQUxP\r\n\
             UjombmJzcDtSJCAxNTAsMDA8L3A+PC9ib2R5PjwvaHRtbD4=\r\n",
        );
        let texto = texto_do_email("PEDIDO.EML", &bytes).unwrap();
        let linhas: Vec<&str> = texto.lines().map(str::trim).collect();
        assert_eq!(
            linhas,
            [
                "Bom dia,",
                "PACIENTE: Maria Conceição",
                "CPF: 529.982.247-25",
                "VALOR: R$ 150,00"
            ],
            "{texto}"
        );
    }

    #[test]
    fn eml_multipart_prefere_texto_puro_e_ignora_anexo() {
        let bytes = eml(
            "Content-Type: multipart/mixed; boundary=\"b1\"",
            "--b1\r\nContent-Type: multipart/alternative; boundary=\"b2\"\r\n\r\n\
             --b2\r\nContent-Type: text/plain; charset=windows-1252\r\nContent-Transfer-Encoding: 8bit\r\n\r\n\
             VALOR: R$ 1.234,56 ~ 07/2026\r\n\
             --b2\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<p>outro</p>\r\n--b2--\r\n\
             --b1\r\nContent-Type: application/pdf; name=\"guia.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0=\r\n--b1--\r\n",
        );
        // 0x96 é o travessão no Windows-1252 (e não é UTF-8 válido): confere a decodificação.
        let bytes: Vec<u8> = bytes.into_iter().map(|b| if b == b'~' { 0x96 } else { b }).collect();
        let texto = texto_do_email("pedido.eml", &bytes).unwrap();
        assert_eq!(texto, "VALOR: R$ 1.234,56 – 07/2026");
    }

    #[test]
    fn eml_corrompido() {
        assert_eq!(texto_do_email("pedido.eml", b"").unwrap_err(), ERRO_ABRIR);
        assert_eq!(
            texto_do_email("pedido.eml", &[0xff, 0xfe, 0x00, 0x01, 0x02]).unwrap_err(),
            ERRO_ABRIR
        );
    }

    #[test]
    fn eml_sem_corpo() {
        let bytes = eml("Content-Type: text/plain; charset=UTF-8", "  \r\n");
        assert_eq!(texto_do_email("pedido.eml", &bytes).unwrap_err(), ERRO_CORPO);
    }

    #[test]
    fn outra_extensao() {
        assert!(texto_do_email("pedido.pdf", b"x").is_err());
        assert!(e_email("Pedido.MSG") && e_email("a.eml") && !e_email("a.xlsx"));
    }
}
