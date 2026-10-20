# Contribuindo

Obrigado por querer ajudar. Antes de abrir um pull request, leia [AGENTS.md](AGENTS.md) (regras do repositório) e [docs/desenvolvimento.md](docs/desenvolvimento.md) (ambiente, testes e convenções). Código, comentários e mensagens são em português do Brasil.

Antes de enviar: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` e os testes dos crates alterados.

## Licença e cessão de direitos

O projeto é distribuído sob a [GNU AGPL-3.0](LICENSE), com licença comercial alternativa oferecida pelo titular dos direitos ([ADR 0016](docs/decisoes/0016-licenca-agpl.md)). Para essa oferta continuar possível, toda contribuição precisa de uma autorização do autor.

Ao enviar um pull request, você declara que:

1. é o autor da contribuição, ou tem o direito de enviá-la, e ela não viola direitos de terceiros nem contém código de licença incompatível com a AGPL-3.0;
2. mantém os direitos autorais sobre o que escreveu;
3. concede a Felipe Canin Novaes, titular do projeto, uma licença perpétua, mundial, gratuita e irrevogável para usar, modificar, sublicenciar e distribuir a contribuição, inclusive sob outras licenças, comerciais ou não, além da AGPL-3.0.

Registre a concordância com a linha abaixo na descrição do pull request:

```
Concordo com a cessão de direitos descrita em CONTRIBUTING.md.
```

Pull requests sem essa linha não serão integrados.

## Dados sensíveis

Nunca envie certificados reais, senhas, bancos, XMLs ou PDFs de clientes, nem em issues. Use os certificados autoassinados de teste que já estão no repositório.
