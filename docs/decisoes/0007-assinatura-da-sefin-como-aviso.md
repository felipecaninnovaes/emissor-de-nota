# 0007 — Assinatura da NFS-e da Sefin conferida só como aviso

Estado: aceita (17/10/2026).

**Contexto.** O relatório de auditoria sugeriu abortar a gravação quando a assinatura devolvida pela Sefin não confere, e validar a cadeia ICP-Brasil.

**Decisão.** `nfse_xml::verificar` continua rodando em `gravar` (`crates/nfse-emissor/src/lib.rs`), mas só gera aviso.

**Por quê.** A nota já foi autorizada e registrada quando a conferência acontece; abortar deixaria uma nota válida sem XML nem PDF e o sistema divergente da Sefin. A resposta já chega autenticada por mTLS. Validar a cadeia exigiria embarcar as ACs raiz.

**Consequências.** A validação da cadeia ICP-Brasil fica como pendência em [escopo.md](../escopo.md).
