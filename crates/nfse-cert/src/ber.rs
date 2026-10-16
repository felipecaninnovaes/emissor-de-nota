//! Conversão de BER para DER nas camadas do `.pfx`.
//!
//! Alguns emissores de certificado (e o Windows, ao exportar) gravam o PKCS#12 com comprimento
//! indefinido (`30 80 ... 00 00`) e OCTET STRING fragmentada, o que é BER válido, mas a leitura
//! do PKCS#12 só aceita DER. Aqui só a moldura muda: o conteúdo de cada valor primitivo (inclusive
//! o trecho protegido pelo MAC e os blocos cifrados) é copiado byte a byte, então o MAC continua
//! conferindo e a senha continua sendo exigida.

/// Profundidade máxima de aninhamento aceita (o PKCS#12 usa uma dezena de níveis).
const PROFUNDIDADE_MAXIMA: usize = 64;

/// Reescreve `ber` em DER. `None` se a estrutura não for ASN.1 válida.
pub(crate) fn para_der(ber: &[u8]) -> Option<Vec<u8>> {
    let mut saida = Vec::with_capacity(ber.len());
    let lido = elemento(ber, &mut saida, 0)?;
    (lido == ber.len()).then_some(saida)
}

/// Converte um elemento a partir do início de `dados`; devolve quantos bytes consumiu.
fn elemento(dados: &[u8], saida: &mut Vec<u8>, profundidade: usize) -> Option<usize> {
    if profundidade > PROFUNDIDADE_MAXIMA {
        return None;
    }
    let tag = *dados.first()?;
    // Tags de número alto (0x1f) não aparecem no PKCS#12.
    if tag & 0x1f == 0x1f {
        return None;
    }
    let construido = tag & 0x20 != 0;
    let (comprimento, mut pos) = ler_comprimento(dados)?;

    if !construido {
        let tamanho = comprimento?; // primitivo não pode ter comprimento indefinido
        let valor = dados.get(pos..pos.checked_add(tamanho)?)?;
        escrever(saida, tag, valor);
        return Some(pos + tamanho);
    }

    let fim = match comprimento {
        Some(n) => Some(pos.checked_add(n).filter(|&f| f <= dados.len())?),
        None => None,
    };
    let mut filhos = Vec::new();
    loop {
        match fim {
            Some(f) if pos == f => break,
            Some(f) if pos > f => return None,
            None if dados.get(pos..pos + 2)? == [0u8, 0] => {
                pos += 2;
                break;
            }
            _ => {}
        }
        let limite = fim.unwrap_or(dados.len());
        pos += elemento(&dados[pos..limite], &mut filhos, profundidade + 1)?;
    }

    if tag == 0x24 {
        // OCTET STRING fragmentada: em DER é uma só, com os pedaços concatenados.
        let valor = valores_de_octet_strings(&filhos)?;
        escrever(saida, 0x04, &valor);
    } else {
        escrever(saida, tag, &filhos);
    }
    Some(pos)
}

/// Concatena os valores de uma sequência de OCTET STRING primitivas (já em DER).
fn valores_de_octet_strings(mut der: &[u8]) -> Option<Vec<u8>> {
    let mut valor = Vec::new();
    while !der.is_empty() {
        if der[0] != 0x04 {
            return None;
        }
        let (tamanho, pos) = ler_comprimento(der)?;
        let tamanho = tamanho?;
        valor.extend_from_slice(der.get(pos..pos + tamanho)?);
        der = &der[pos + tamanho..];
    }
    Some(valor)
}

/// Lê o comprimento depois da tag. `None` no primeiro campo = comprimento indefinido.
fn ler_comprimento(dados: &[u8]) -> Option<(Option<usize>, usize)> {
    let primeiro = *dados.get(1)?;
    if primeiro < 0x80 {
        return Some((Some(primeiro as usize), 2));
    }
    if primeiro == 0x80 {
        return Some((None, 2));
    }
    let n = (primeiro & 0x7f) as usize;
    if n > std::mem::size_of::<usize>() {
        return None;
    }
    let tamanho = dados
        .get(2..2 + n)?
        .iter()
        .fold(0usize, |acc, &b| (acc << 8) | b as usize);
    Some((Some(tamanho), 2 + n))
}

fn escrever(saida: &mut Vec<u8>, tag: u8, valor: &[u8]) {
    saida.push(tag);
    let n = valor.len();
    if n < 0x80 {
        saida.push(n as u8);
    } else {
        let bytes = n.to_be_bytes();
        let inicio = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len() - 1);
        saida.push(0x80 | (bytes.len() - inicio) as u8);
        saida.extend_from_slice(&bytes[inicio..]);
    }
    saida.extend_from_slice(valor);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn der_continua_igual() {
        let der = [0x30, 0x06, 0x02, 0x01, 0x03, 0x04, 0x01, 0xaa];
        assert_eq!(para_der(&der).unwrap(), der);
    }

    #[test]
    fn comprimento_indefinido_e_octet_string_fragmentada() {
        // SEQUENCE indefinida { INTEGER 3, [0] indefinido { OCTET STRING fragmentada "ab" + "c" } }
        let ber = [
            0x30, 0x80, 0x02, 0x01, 0x03, 0xa0, 0x80, 0x24, 0x80, 0x04, 0x02, b'a', b'b', 0x04, 0x01, b'c', 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        let der = [0x30, 0x0a, 0x02, 0x01, 0x03, 0xa0, 0x05, 0x04, 0x03, b'a', b'b', b'c'];
        assert_eq!(para_der(&ber).unwrap(), der);
    }

    #[test]
    fn comprimento_longo() {
        let valor = vec![7u8; 300];
        let mut ber = vec![0x30, 0x80, 0x04, 0x82, 0x01, 0x2c];
        ber.extend_from_slice(&valor);
        ber.extend_from_slice(&[0, 0]);
        let der = para_der(&ber).unwrap();
        assert_eq!(&der[..8], &[0x30, 0x82, 0x01, 0x30, 0x04, 0x82, 0x01, 0x2c]);
        assert_eq!(der.len(), 4 + 4 + 300);
    }

    #[test]
    fn recusa_lixo() {
        assert!(para_der(&[0x30, 0x80, 0x02, 0x01]).is_none());
        assert!(para_der(&[0x30, 0x05, 0x02, 0x01, 0x03]).is_none());
        assert!(para_der(&[0x04, 0x80, 0x00, 0x00]).is_none());
        assert!(para_der(&[]).is_none());
    }
}
