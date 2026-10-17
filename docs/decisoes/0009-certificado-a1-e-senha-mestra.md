# 0009 — Somente certificado A1 e senha mestra sem perfis

Estado: aceita (16/10/2026).

**Contexto.** 90% dos clientes já têm e-CNPJ A1; cada máquina é de um operador.

**Decisão.** Só A1 (`.pfx`/`.p12`); A3/PKCS#11, DPAPI e CNG ficam fora do escopo. O acesso é por uma senha mestra por máquina, sem recuperação, que deriva a chave do cofre (Argon2id + AES-256-GCM). Sem usuários, perfis nem restrição por empresa.

**Consequências.** Perder a senha mestra obriga a reimportar os certificados. Ver [arquitetura.md](../arquitetura.md#cofre-de-certificados).
