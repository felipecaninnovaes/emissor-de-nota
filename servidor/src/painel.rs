//! Painel do administrador: login, clientes, tokens e backups. HTML gerado no servidor, sem JavaScript.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use askama::Template;
use axum::Form;
use axum::Router;
use axum::extract::{ConnectInfo, FromRequestParts, Path, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use chrono::{DateTime, FixedOffset, Utc};
use serde::Deserialize;

use crate::banco::{self, DadosCliente};
use crate::{Compartilhado, api, token};

const COOKIE: &str = "sessao";
const DURACAO_SESSAO: chrono::Duration = chrono::Duration::hours(12);
/// Tentativas de login erradas aceitas por origem dentro de [`JANELA_TENTATIVAS`].
const MAXIMO_TENTATIVAS: u32 = 5;
const JANELA_TENTATIVAS: Duration = Duration::from_secs(15 * 60);

pub fn rotas(estado: Compartilhado) -> Router<Compartilhado> {
    Router::new()
        .route("/", get(lista))
        .route("/login", get(tela_login).post(entrar))
        .route("/sair", post(sair))
        .route("/clientes/novo", get(novo).post(criar))
        .route("/clientes/{id}", get(detalhes))
        .route("/clientes/{id}/editar", get(editar).post(salvar))
        .route("/clientes/{id}/ativo", post(alternar_ativo))
        .route("/clientes/{id}/token", post(novo_token))
        .route("/clientes/{id}/excluir", get(confirmar_exclusao).post(excluir))
        .route("/clientes/{id}/backups/{backup}", get(baixar))
        .route("/clientes/{id}/backups/{backup}/excluir", post(excluir_backup))
        .layer(middleware::from_fn_with_state(estado, conferir_origem))
}

// ---------- utilidades ----------

/// Página de erro do painel.
pub struct ErroPainel(StatusCode, String);

impl IntoResponse for ErroPainel {
    fn into_response(self) -> Response {
        let pagina = PaginaErro {
            titulo: "Erro",
            logado: true,
            mensagem: &self.1,
        };
        (self.0, Html(pagina.render().unwrap_or(self.1))).into_response()
    }
}

fn interno(e: impl std::fmt::Display) -> ErroPainel {
    tracing::error!("{e}");
    ErroPainel(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Erro interno. Veja o log do container do painel.".into(),
    )
}

fn nao_encontrado() -> ErroPainel {
    ErroPainel(StatusCode::NOT_FOUND, "Não encontrado.".into())
}

fn pagina(t: impl Template) -> Result<Html<String>, ErroPainel> {
    t.render().map(Html).map_err(interno)
}

/// Data e hora de Brasília (UTC−3, sem horário de verão).
pub fn data_br(d: DateTime<Utc>) -> String {
    let brasilia = FixedOffset::west_opt(3 * 3600).expect("fuso válido");
    d.with_timezone(&brasilia).format("%d/%m/%Y %H:%M").to_string()
}

pub fn tamanho_legivel(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    let (valor, unidade) = if b >= 1024.0 * 1024.0 * 1024.0 {
        (b / (1024.0 * 1024.0 * 1024.0), "GB")
    } else if b >= 1024.0 * 1024.0 {
        (b / (1024.0 * 1024.0), "MB")
    } else {
        (b / 1024.0, "KB")
    };
    format!("{valor:.1} {unidade}").replace('.', ",")
}

/// `Origin` do formulário igual ao endereço do painel: impede que outro site poste com a sessão do admin.
pub fn origem_confere(origem: Option<&str>, host: Option<&str>, url_publica: Option<&str>) -> bool {
    let Some(origem) = origem.map(|o| o.trim_end_matches('/')) else {
        return false;
    };
    match url_publica {
        Some(u) => origem.eq_ignore_ascii_case(u),
        None => origem
            .split_once("://")
            .is_some_and(|(_, h)| host.is_some_and(|host| h.eq_ignore_ascii_case(host))),
    }
}

async fn conferir_origem(State(estado): State<Compartilhado>, req: Request, next: Next) -> Response {
    if req.method() == Method::POST {
        let h = req.headers();
        let texto = |nome: &str| h.get(nome).and_then(|v| v.to_str().ok());
        let host = texto("x-forwarded-host").or(texto(header::HOST.as_str()));
        if !origem_confere(
            texto(header::ORIGIN.as_str()),
            host,
            estado.config.url_publica.as_deref(),
        ) {
            return ErroPainel(
                StatusCode::FORBIDDEN,
                "Formulário de outra origem recusado. Se o painel fica atrás de um proxy, confira URL_PUBLICA.".into(),
            )
            .into_response();
        }
    }
    next.run(req).await
}

fn ler_cookie<'a>(cabecalhos: &'a HeaderMap, nome: &str) -> Option<&'a str> {
    cabecalhos
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|par| par.trim().split_once('='))
        .find(|(n, _)| *n == nome)
        .map(|(_, v)| v)
}

/// Sessão do administrador válida; sem ela, volta para o login.
pub struct Admin;

impl FromRequestParts<Compartilhado> for Admin {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, estado: &Compartilhado) -> Result<Self, Self::Rejection> {
        let Some(id) = ler_cookie(&parts.headers, COOKIE) else {
            return Err(Redirect::to("/login").into_response());
        };
        match banco::sessao_valida(&estado.banco, &token::hash(id)).await {
            Ok(true) => Ok(Admin),
            Ok(false) => Err(Redirect::to("/login").into_response()),
            Err(e) => Err(interno(e).into_response()),
        }
    }
}

/// Quem está tentando entrar: o primeiro `X-Forwarded-For` (atrás do proxy) ou o endereço da conexão.
struct Origem(String);

impl<S: Send + Sync> FromRequestParts<S> for Origem {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let encaminhado = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_owned());
        let conexao = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(Origem(encaminhado.or(conexao).unwrap_or_default()))
    }
}

// ---------- templates ----------

#[derive(Template)]
#[template(path = "erro.html")]
struct PaginaErro<'a> {
    titulo: &'a str,
    logado: bool,
    mensagem: &'a str,
}

#[derive(Template)]
#[template(path = "login.html")]
struct Login<'a> {
    titulo: &'a str,
    logado: bool,
    erro: Option<&'a str>,
}

struct LinhaCliente {
    id: i64,
    nome: String,
    documento: String,
    ativo: bool,
    tem_token: bool,
    backups: i64,
    espaco: String,
    ultimo: String,
}

#[derive(Template)]
#[template(path = "clientes.html")]
struct Lista<'a> {
    titulo: &'a str,
    logado: bool,
    clientes: Vec<LinhaCliente>,
    total_espaco: String,
}

#[derive(Template)]
#[template(path = "cliente_form.html")]
struct FormCliente<'a> {
    titulo: &'a str,
    logado: bool,
    acao: String,
    dados: DadosCliente,
    erro: Option<&'a str>,
}

struct LinhaBackup {
    id: i64,
    nome: String,
    tamanho: String,
    sha256: String,
    criado_em: String,
}

#[derive(Template)]
#[template(path = "cliente.html")]
struct Detalhes<'a> {
    titulo: &'a str,
    logado: bool,
    cliente: banco::Cliente,
    criado_em: String,
    backups: Vec<LinhaBackup>,
    espaco: String,
    mantidos: i64,
}

#[derive(Template)]
#[template(path = "token.html")]
struct TokenGerado<'a> {
    titulo: &'a str,
    logado: bool,
    cliente_id: i64,
    nome: String,
    token: String,
}

#[derive(Template)]
#[template(path = "excluir.html")]
struct ConfirmarExclusao<'a> {
    titulo: &'a str,
    logado: bool,
    cliente: banco::Cliente,
    backups: usize,
    erro: Option<&'a str>,
}

// ---------- login ----------

#[derive(Deserialize)]
struct DadosLogin {
    usuario: String,
    senha: String,
}

async fn tela_login() -> Result<Html<String>, ErroPainel> {
    pagina(Login {
        titulo: "Entrar",
        logado: false,
        erro: None,
    })
}

fn senha_confere(hash: &str, senha: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash).is_ok_and(|h| argon2::Argon2::default().verify_password(senha.as_bytes(), &h).is_ok())
}

async fn entrar(
    State(estado): State<Compartilhado>,
    Origem(origem): Origem,
    Form(dados): Form<DadosLogin>,
) -> Result<Response, ErroPainel> {
    let bloqueado = {
        let mut t = estado.tentativas.lock().expect("trava das tentativas");
        t.retain(|_, (_, inicio)| inicio.elapsed() < JANELA_TENTATIVAS);
        t.get(&origem).is_some_and(|(n, _)| *n >= MAXIMO_TENTATIVAS)
    };
    if bloqueado {
        let html = pagina(Login {
            titulo: "Entrar",
            logado: false,
            erro: Some("Muitas tentativas erradas. Espere 15 minutos e tente de novo."),
        })?;
        return Ok((StatusCode::TOO_MANY_REQUESTS, html).into_response());
    }
    // A senha é conferida mesmo com usuário errado, para o tempo de resposta não revelar o usuário.
    let senha_ok = senha_confere(&estado.config.admin_senha_hash, &dados.senha);
    if !(senha_ok && dados.usuario.trim() == estado.config.admin_usuario) {
        estado
            .tentativas
            .lock()
            .expect("trava das tentativas")
            .entry(origem.clone())
            .or_insert((0, Instant::now()))
            .0 += 1;
        tracing::warn!(origem, "login recusado");
        let html = pagina(Login {
            titulo: "Entrar",
            logado: false,
            erro: Some("Usuário ou senha incorretos."),
        })?;
        return Ok((StatusCode::UNAUTHORIZED, html).into_response());
    }
    estado.tentativas.lock().expect("trava das tentativas").remove(&origem);
    let id = token::aleatorio();
    banco::criar_sessao(&estado.banco, &token::hash(&id), Utc::now() + DURACAO_SESSAO)
        .await
        .map_err(interno)?;
    let cookie = format!(
        "{COOKIE}={id}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        DURACAO_SESSAO.num_seconds(),
        if estado.config.cookie_seguro { "; Secure" } else { "" }
    );
    let mut resposta = Redirect::to("/").into_response();
    resposta
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).map_err(interno)?);
    Ok(resposta)
}

async fn sair(State(estado): State<Compartilhado>, cabecalhos: HeaderMap) -> Result<Response, ErroPainel> {
    if let Some(id) = ler_cookie(&cabecalhos, COOKIE) {
        banco::apagar_sessao(&estado.banco, &token::hash(id))
            .await
            .map_err(interno)?;
    }
    let mut resposta = Redirect::to("/login").into_response();
    resposta.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("sessao=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"),
    );
    Ok(resposta)
}

// ---------- clientes ----------

async fn lista(State(estado): State<Compartilhado>, _: Admin) -> Result<Html<String>, ErroPainel> {
    let clientes = banco::listar_clientes(&estado.banco).await.map_err(interno)?;
    let total: i64 = clientes.iter().map(|c| c.espaco).sum();
    pagina(Lista {
        titulo: "Clientes",
        logado: true,
        total_espaco: tamanho_legivel(total),
        clientes: clientes
            .into_iter()
            .map(|c| LinhaCliente {
                id: c.id,
                nome: c.nome,
                documento: c.documento,
                ativo: c.ativo,
                tem_token: c.tem_token,
                backups: c.backups,
                espaco: tamanho_legivel(c.espaco),
                ultimo: c.ultimo.map(data_br).unwrap_or_else(|| "—".into()),
            })
            .collect(),
    })
}

async fn novo(_: Admin) -> Result<Html<String>, ErroPainel> {
    pagina(FormCliente {
        titulo: "Novo cliente",
        logado: true,
        acao: "/clientes/novo".into(),
        dados: DadosCliente::default(),
        erro: None,
    })
}

fn token_gerado(cliente_id: i64, nome: String, token: String) -> Result<Html<String>, ErroPainel> {
    pagina(TokenGerado {
        titulo: "Token do cliente",
        logado: true,
        cliente_id,
        nome,
        token,
    })
}

async fn criar(
    State(estado): State<Compartilhado>,
    _: Admin,
    Form(dados): Form<DadosCliente>,
) -> Result<Response, ErroPainel> {
    if dados.nome.trim().is_empty() {
        return Ok(pagina(FormCliente {
            titulo: "Novo cliente",
            logado: true,
            acao: "/clientes/novo".into(),
            dados,
            erro: Some("Informe o nome do cliente."),
        })?
        .into_response());
    }
    let token = token::gerar();
    let id = banco::criar_cliente(&estado.banco, &dados, &token::hash(&token))
        .await
        .map_err(interno)?;
    tracing::info!(cliente = id, "cliente criado");
    Ok(token_gerado(id, dados.nome.trim().to_owned(), token)?.into_response())
}

async fn carregar(estado: &Compartilhado, id: i64) -> Result<banco::Cliente, ErroPainel> {
    banco::cliente(&estado.banco, id)
        .await
        .map_err(interno)?
        .ok_or_else(nao_encontrado)
}

async fn detalhes(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
) -> Result<Html<String>, ErroPainel> {
    let cliente = carregar(&estado, id).await?;
    let backups = banco::backups(&estado.banco, id).await.map_err(interno)?;
    let espaco = backups.iter().map(|b| b.tamanho).sum();
    pagina(Detalhes {
        titulo: "Cliente",
        logado: true,
        criado_em: data_br(cliente.criado_em),
        cliente,
        espaco: tamanho_legivel(espaco),
        mantidos: estado.config.manter_backups,
        backups: backups
            .into_iter()
            .map(|b| LinhaBackup {
                id: b.id,
                nome: b.nome,
                tamanho: tamanho_legivel(b.tamanho),
                sha256: b.sha256.chars().take(12).collect(),
                criado_em: data_br(b.criado_em),
            })
            .collect(),
    })
}

async fn editar(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
) -> Result<Html<String>, ErroPainel> {
    let c = carregar(&estado, id).await?;
    pagina(FormCliente {
        titulo: "Editar cliente",
        logado: true,
        acao: format!("/clientes/{id}/editar"),
        dados: DadosCliente {
            nome: c.nome,
            documento: c.documento,
            email: c.email,
            observacao: c.observacao,
        },
        erro: None,
    })
}

async fn salvar(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
    Form(dados): Form<DadosCliente>,
) -> Result<Response, ErroPainel> {
    carregar(&estado, id).await?;
    if dados.nome.trim().is_empty() {
        return Ok(pagina(FormCliente {
            titulo: "Editar cliente",
            logado: true,
            acao: format!("/clientes/{id}/editar"),
            dados,
            erro: Some("Informe o nome do cliente."),
        })?
        .into_response());
    }
    banco::atualizar_cliente(&estado.banco, id, &dados)
        .await
        .map_err(interno)?;
    Ok(Redirect::to(&format!("/clientes/{id}")).into_response())
}

#[derive(Deserialize)]
struct DadosAtivo {
    ativo: bool,
}

async fn alternar_ativo(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
    Form(dados): Form<DadosAtivo>,
) -> Result<Redirect, ErroPainel> {
    carregar(&estado, id).await?;
    banco::definir_ativo(&estado.banco, id, dados.ativo)
        .await
        .map_err(interno)?;
    tracing::info!(cliente = id, ativo = dados.ativo, "situação do cliente alterada");
    Ok(Redirect::to(&format!("/clientes/{id}")))
}

/// Gera outro token: o anterior deixa de valer na hora.
async fn novo_token(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
) -> Result<Html<String>, ErroPainel> {
    let c = carregar(&estado, id).await?;
    let token = token::gerar();
    banco::definir_token(&estado.banco, id, &token::hash(&token))
        .await
        .map_err(interno)?;
    tracing::info!(cliente = id, "token trocado");
    token_gerado(id, c.nome, token)
}

async fn confirmar_exclusao(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
) -> Result<Html<String>, ErroPainel> {
    let cliente = carregar(&estado, id).await?;
    let backups = banco::backups(&estado.banco, id).await.map_err(interno)?.len();
    pagina(ConfirmarExclusao {
        titulo: "Excluir cliente",
        logado: true,
        cliente,
        backups,
        erro: None,
    })
}

#[derive(Deserialize)]
struct DadosExclusao {
    confirmacao: String,
}

/// Apaga os arquivos do S3 e depois o cliente. Exige digitar o nome, como confirmação.
async fn excluir(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path(id): Path<i64>,
    Form(dados): Form<DadosExclusao>,
) -> Result<Response, ErroPainel> {
    let cliente = carregar(&estado, id).await?;
    let backups = banco::backups(&estado.banco, id).await.map_err(interno)?;
    if dados.confirmacao.trim() != cliente.nome {
        return Ok(pagina(ConfirmarExclusao {
            titulo: "Excluir cliente",
            logado: true,
            cliente,
            backups: backups.len(),
            erro: Some("O nome digitado não confere."),
        })?
        .into_response());
    }
    for b in &backups {
        estado.armazenamento.apagar(&b.chave).await.map_err(interno)?;
    }
    banco::excluir_cliente(&estado.banco, id).await.map_err(interno)?;
    tracing::info!(cliente = id, backups = backups.len(), "cliente excluído");
    Ok(Redirect::to("/").into_response())
}

async fn baixar(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path((id, backup)): Path<(i64, i64)>,
) -> Result<Response, ErroPainel> {
    let b = banco::backup(&estado.banco, id, backup)
        .await
        .map_err(interno)?
        .ok_or_else(nao_encontrado)?;
    api::resposta_download(&estado, &b).await.map_err(interno)
}

async fn excluir_backup(
    State(estado): State<Compartilhado>,
    _: Admin,
    Path((id, backup)): Path<(i64, i64)>,
) -> Result<Redirect, ErroPainel> {
    let b = banco::backup(&estado.banco, id, backup)
        .await
        .map_err(interno)?
        .ok_or_else(nao_encontrado)?;
    estado.armazenamento.apagar(&b.chave).await.map_err(interno)?;
    banco::apagar_backup(&estado.banco, b.id).await.map_err(interno)?;
    Ok(Redirect::to(&format!("/clientes/{id}")))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn origem_do_formulario() {
        let url = Some("https://backup.exemplo.com.br");
        assert!(origem_confere(Some("https://backup.exemplo.com.br"), None, url));
        assert!(!origem_confere(Some("https://outro.com"), None, url));
        assert!(!origem_confere(None, Some("backup.exemplo.com.br"), url));
        // Sem URL_PUBLICA: compara com o Host da requisição.
        assert!(origem_confere(
            Some("http://localhost:8080"),
            Some("localhost:8080"),
            None
        ));
        assert!(!origem_confere(Some("http://evil.test"), Some("localhost:8080"), None));
        assert!(!origem_confere(Some("null"), Some("localhost:8080"), None));
    }

    #[test]
    fn tamanho_e_data_em_portugues() {
        assert_eq!(tamanho_legivel(512), "0,5 KB");
        assert_eq!(tamanho_legivel(5 * 1024 * 1024 + 300 * 1024), "5,3 MB");
        let d = DateTime::parse_from_rfc3339("2026-10-27T13:05:00Z").unwrap().to_utc();
        assert_eq!(data_br(d), "27/10/2026 10:05");
    }

    #[test]
    fn cookie_entre_varios() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("a=1; sessao=abc; b=2"));
        assert_eq!(ler_cookie(&h, "sessao"), Some("abc"));
        assert_eq!(ler_cookie(&h, "c"), None);
    }
}
