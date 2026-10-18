//! Utilitários de texto para comparar o que o usuário digita com cadastros e listas.

/// Minúsculas e sem acento: "Serviço Médico" → "servico medico". Espaços e pontuação ficam.
pub fn sem_acento_minusculo(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tira_acentos_e_caixa_alta() {
        assert_eq!(sem_acento_minusculo("SERVIÇO Médico Ótico"), "servico medico otico");
        assert_eq!(sem_acento_minusculo("Àèìòü Ñ"), "aeiou n");
    }
}
