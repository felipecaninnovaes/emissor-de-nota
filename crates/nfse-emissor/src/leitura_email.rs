//! Leitura do corpo de um e-mail salvo (`.eml` ou `.msg`) em texto, para a importação de pedidos
//! de nota.
//!
//! Do `.eml` (MIME) vale o primeiro corpo em texto puro; sem ele, o HTML convertido em texto. A
//! decodificação (quoted-printable, base64 e charset) é do `mail-parser`. Do `.msg` do Outlook
//! (contêiner CFB) só os streams do corpo são lidos. Os anexos são ignorados.
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
        "msg" => ler_msg(bytes),
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

/// Corpo de um `.msg` do Outlook (contêiner CFB com uma propriedade MAPI por stream).
///
/// Lê só os streams do corpo: `PR_BODY` em Unicode (`1000001F`) ou na página de código da
/// mensagem (`1000001E`) e, sem eles, `PR_HTML` (`1013`). O corpo só em RTF compactado (`1009`)
/// não é lido.
fn ler_msg(bytes: &[u8]) -> Result<String, String> {
    let mut cfb = cfb::CompoundFile::open(std::io::Cursor::new(bytes)).map_err(|_| ERRO_ABRIR.to_string())?;
    let e_mensagem = cfb
        .read_root_storage()
        .any(|e| e.is_stream() && e.name().starts_with("__substg1.0_"));
    if !e_mensagem {
        return Err(ERRO_ABRIR.into());
    }
    let mut ler = |nome: &str| -> Option<Vec<u8>> {
        let mut stream = cfb.open_stream(format!("/{nome}")).ok()?;
        let mut conteudo = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut conteudo).ok()?;
        Some(conteudo)
    };
    let codificacao = ler("__properties_version1.0")
        .and_then(|p| pagina_de_codigo(&p))
        .map_or(encoding_rs::WINDOWS_1252, codificacao_da_pagina);

    let texto = if let Some(b) = ler("__substg1.0_1000001F") {
        arrumar_linhas(&utf16(&b))
    } else if let Some(b) = ler("__substg1.0_1000001E") {
        arrumar_linhas(&codificacao.decode(&b).0)
    } else if let Some(b) = ler("__substg1.0_1013001F") {
        html_em_texto(&utf16(&b))
    } else if let Some(b) = ler("__substg1.0_10130102") {
        // O HTML binário costuma vir em UTF-8 mesmo com outra página de código declarada.
        let html = match std::str::from_utf8(&b) {
            Ok(t) => t.to_string(),
            Err(_) => codificacao.decode(&b).0.into_owned(),
        };
        html_em_texto(&html)
    } else {
        return Err(ERRO_CORPO.into());
    };
    let texto = texto.trim_end_matches('\0').to_string();
    if texto.trim().is_empty() {
        return Err(ERRO_CORPO.into());
    }
    Ok(texto)
}

/// UTF-16LE do Outlook, sem o terminador nulo.
fn utf16(bytes: &[u8]) -> String {
    let unidades: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&unidades).trim_end_matches('\0').to_string()
}

/// Página de código da mensagem em `__properties_version1.0`: `PR_INTERNET_CPID` (`3FDE`) ou,
/// sem ela, `PR_MESSAGE_CODEPAGE` (`3FFD`). Cabeçalho de 32 bytes e entradas de 16 bytes
/// (tag, flags, valor).
fn pagina_de_codigo(propriedades: &[u8]) -> Option<u32> {
    let valor = |tag: u32| {
        propriedades.get(32..)?.chunks_exact(16).find_map(|e| {
            let t = u32::from_le_bytes(e[0..4].try_into().ok()?);
            (t == tag).then(|| u32::from_le_bytes(e[8..12].try_into().unwrap_or_default()))
        })
    };
    valor(0x3FDE_0003).or_else(|| valor(0x3FFD_0003))
}

/// Codificação de uma página de código do Windows; desconhecida vale Windows-1252.
fn codificacao_da_pagina(pagina: u32) -> &'static encoding_rs::Encoding {
    match pagina {
        65001 => encoding_rs::UTF_8,
        1250 => encoding_rs::WINDOWS_1250,
        28592 => encoding_rs::ISO_8859_2,
        28605 => encoding_rs::ISO_8859_15,
        // 1252, 28591 (ISO-8859-1, que o encoding_rs trata como 1252), 20127 (ASCII) e o resto.
        _ => encoding_rs::WINDOWS_1252,
    }
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

    /// `.msg` fictício: um contêiner CFB com os streams dados (nome sem a barra inicial).
    fn msg(streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        let mut cfb = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        for (nome, conteudo) in streams {
            let mut s = cfb.create_stream(format!("/{nome}")).unwrap();
            s.write_all(conteudo).unwrap();
        }
        cfb.flush().unwrap();
        cfb.into_inner().into_inner()
    }

    fn utf16(texto: &str) -> Vec<u8> {
        texto.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
    }

    /// `__properties_version1.0` da mensagem com uma propriedade inteira (tag, valor).
    fn propriedades(tag: u32, valor: u32) -> Vec<u8> {
        let mut p = vec![0u8; 32];
        p.extend(tag.to_le_bytes());
        p.extend(6u32.to_le_bytes());
        p.extend(valor.to_le_bytes());
        p.extend([0u8; 4]);
        p
    }

    const CORPO: &str = "Bom dia,\r\n\r\nPACIENTE: João Conceição   CPF: 529.982.247-25\r\nVALOR: R$ 150,00\r\nCIDADE/UF: São José do Rio Preto/SP";

    #[test]
    fn msg_unicode() {
        let bytes = msg(&[
            ("__substg1.0_0037001F", utf16("Notas de outubro")),
            ("__substg1.0_1000001F", utf16(CORPO)),
            ("__substg1.0_10090102", vec![1, 2, 3]),
        ]);
        let texto = texto_do_email("pedido.msg", &bytes).unwrap();
        assert_eq!(texto, CORPO.replace("\r\n", "\n"));
    }

    #[test]
    fn msg_ansi_na_pagina_de_codigo_declarada() {
        let (ansi, _, _) = encoding_rs::WINDOWS_1252.encode(CORPO);
        let bytes = msg(&[
            ("__properties_version1.0", propriedades(0x3FFD_0003, 1252)),
            ("__substg1.0_1000001E", ansi.into_owned()),
        ]);
        assert_eq!(
            texto_do_email("PEDIDO.MSG", &bytes).unwrap(),
            CORPO.replace("\r\n", "\n")
        );
        // UTF-8 declarado pelo PR_INTERNET_CPID.
        let bytes = msg(&[
            ("__properties_version1.0", propriedades(0x3FDE_0003, 65001)),
            ("__substg1.0_1000001E", CORPO.as_bytes().to_vec()),
        ]);
        assert_eq!(
            texto_do_email("pedido.msg", &bytes).unwrap(),
            CORPO.replace("\r\n", "\n")
        );
    }

    #[test]
    fn msg_so_html() {
        let html = "<html><body><div>PACIENTE: Jos&eacute;</div><div>VALOR: R$ 10,00</div></body></html>";
        let bytes = msg(&[("__substg1.0_10130102", html.as_bytes().to_vec())]);
        assert_eq!(
            texto_do_email("pedido.msg", &bytes).unwrap(),
            "PACIENTE: José\nVALOR: R$ 10,00"
        );
    }

    #[test]
    fn msg_so_rtf() {
        let bytes = msg(&[
            ("__substg1.0_0037001F", utf16("Notas")),
            ("__substg1.0_10090102", vec![0x10, 0, 0, 0, 0x20]),
        ]);
        assert_eq!(texto_do_email("pedido.msg", &bytes).unwrap_err(), ERRO_CORPO);
    }

    #[test]
    fn msg_corrompido() {
        assert_eq!(texto_do_email("pedido.msg", b"nao e um msg").unwrap_err(), ERRO_ABRIR);
        // CFB válido que não é mensagem (um .doc renomeado, por exemplo).
        let bytes = msg(&[("WordDocument", vec![0; 16])]);
        assert_eq!(texto_do_email("pedido.msg", &bytes).unwrap_err(), ERRO_ABRIR);
    }

    #[test]
    fn outra_extensao() {
        assert!(texto_do_email("pedido.pdf", b"x").is_err());
        assert!(e_email("Pedido.MSG") && e_email("a.eml") && !e_email("a.xlsx"));
    }
}
