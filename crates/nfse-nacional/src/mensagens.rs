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
        "E0014" => {
            "Esta série e número já geraram uma NFS-e (por este ou por outro emissor). Em Empresas → \
             \"Numeração / IM\", clique em \"Consultar na Sefin\" para pular os números já usados e depois \
             use \"Corrigir e emitir\" nesta nota."
        }
        "E0312" => {
            "O município não habilitou este código de tributação nacional para a empresa neste ambiente. \
             Confira o código do serviço (cTribNac) com a prefeitura; em Produção Restrita a parametrização \
             do município pode ser diferente da de Produção."
        }
        "E0116" => {
            "O município exige a inscrição municipal na DPS. Preencha a IM em Empresas → Editar e deixe \
             marcado \"informar IM na DPS\" para este ambiente em \"Numeração / IM\"."
        }
        "E0710" | "E0712" | "E0713" => {
            "A forma do total aproximado de tributos não combina com a situação no Simples. Em Serviços, \
             edite o serviço: ME/EPP usa \"% do Simples Nacional\"; não optante usa \"% federal / \
             estadual / municipal\"; MEI não usa o % do Simples."
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
        "E0617" => {
            "A alíquota do ISS não pode ir na DPS de prestador não optante do Simples Nacional: a Sefin              usa a do município. Se a nota foi emitida por versão anterior à 1.8.1, atualize o emissor;              senão, informe o suporte."
        }
        "E1235" => {
            "A Sefin recusou a estrutura do XML da DPS (o texto mostra o elemento fora de lugar).              Costuma ser o leiaute da DPS: em Configurações → Leiaute da DPS, troque entre NT 009 e NT 004              para o ambiente da nota e emita de novo; se persistir, informe o suporte."
        }
        "E2400" => "Identificador da DPS mal formado. Erro do emissor: informe o suporte.",
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
        assert!(orientacao("E0617").unwrap().contains("não optante"));
        assert!(orientacao("E1235").unwrap().contains("Leiaute da DPS"));
    }
}
