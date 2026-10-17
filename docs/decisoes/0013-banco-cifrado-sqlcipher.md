# 0013 — Banco cifrado com SQLCipher

Estado: aceita (17/10/2026). Substitui a recomendação anterior de adiar o SQLCipher ([escopo.md](../escopo.md)).

**Contexto.** O `emissor.db` guarda nomes, CPF/CNPJ, e-mails, valores e os XMLs das notas. Só os certificados e a senha do SMTP eram cifrados. O software ainda não tinha instalações reais, então não havia bancos em uso para migrar.

**Decisão.** O arquivo inteiro é cifrado com SQLCipher (`libsqlite3-sys` com `bundled-sqlcipher-vendored-openssl`; `sqlx` com `sqlite-unbundled`). A chave vem do mesmo Argon2id da senha mestra: o resultado serve de chave AES dos certificados e, por SHA-256 com um rótulo, de chave bruta do SQLCipher (sem o PBKDF2 do SQLCipher). O sal, os parâmetros e o verificador saem da tabela `configuracao` e vão para `emissor.cofre`, ao lado do banco, porque o banco não abre sem a chave. A aparência da interface vai para `aparencia.json` pelo mesmo motivo (é lida antes da senha). O banco só é aberto depois da senha; "Bloquear" o fecha.

**Por quê.** Protege o arquivo parado em disco, em cópias soltas e em backups mal guardados, sem depender de o operador ligar a criptografia de disco. Uma única senha continua valendo para tudo.

**Consequências.**
- Perder a senha mestra, ou o `emissor.cofre`, perde o banco (já era assim para os certificados).
- Copiar o banco exige copiar os dois arquivos. O backup `.nfbk` leva o cabeçalho dentro e a restauração gera os dois.
- Bancos em claro de versões anteriores são convertidos na primeira abertura, com a mesma senha (a chave dos certificados não muda, nada precisa ser reimportado); o original em claro fica como `emissor.db.legado` e deve ser apagado depois de conferir.
- A compilação precisa de compilador C e Perl (OpenSSL vendored). No Windows, o `libsqlite3-sys` também precisa estar em `[build-dependencies]` do `nfse-db` (que já tem `build.rs`, para as migrações): sem isso o build de host dos macros do `sqlx` compila o crate sem as features `bundled` e tenta linkar o SQLite do sistema (falha com `sqlite3.h not found`). Verificado em CI no Windows e no Linux.
- As migrações do sqlx e os ajustes usam `&mut conexão`, e o rustc não prova `Send` para isso dentro de um comando do Tauri; a abertura roda numa thread bloqueante (`em_thread_bloqueante` em `comandos/cofre.rs`).
- Não protege com o aplicativo aberto e desbloqueado, nem os XML e PDF da pasta de saída (usar criptografia de disco).
- Não há troca de senha mestra (exigiria `PRAGMA rekey` e recifrar os certificados).
