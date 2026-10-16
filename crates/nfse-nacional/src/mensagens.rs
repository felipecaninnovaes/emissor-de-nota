//! O que fazer diante dos códigos de rejeição já vistos na prática.
//!
//! O texto da Sefin vai sempre junto; aqui entra só a orientação para o operador.
//! Acrescentar códigos conforme aparecerem (com a situação real que os provocou).

/// Orientação para o operador sobre um código de rejeição da Sefin, se conhecido.
pub fn orientacao(codigo: &str) -> Option<&'static str> {
    Some(match codigo {
        "E0084" => {
            "O cadastro nacional (CNC) não tem a empresa estabelecida no município emissor nesta data. \
             Em Produção Restrita isso acontece quando o município não carregou o cadastro do contribuinte; \
             em Produção, confira o código IBGE do município emissor e a data de competência."
        }
        "E0120" => {
            "A inscrição municipal não deve ser enviada neste ambiente. \
             Desmarque \"informar IM na DPS\" para este ambiente no cadastro da empresa."
        }
        "E0714" | "E0717" => {
            "A Sefin recusou a assinatura ou o certificado. Rode o diagnóstico do certificado \
             (validade, AC, uso Client Authentication) e confira se é o e-CNPJ da própria empresa."
        }
        "E1228" => "Assinatura com prefixo de namespace (ds:). Erro do emissor: informe o suporte.",
        "E1235" | "E2400" => "Identificador da DPS mal formado. Erro do emissor: informe o suporte.",
        "E2404" => "Não há NFS-e gerada para esta DPS.",
        _ => return None,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_conhecidos() {
        assert!(orientacao("E0120").unwrap().contains("inscrição municipal"));
        assert!(orientacao("E9999").is_none());
    }
}
