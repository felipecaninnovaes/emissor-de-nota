//! Testes da API e do painel contra um Postgres de verdade (`DATABASE_URL`), com o armazenamento em memória.
//! Rodam no CI com `cargo test -- --include-ignored`; o sqlx cria um banco novo para cada teste.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use bytes::Bytes;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::armazenamento::Armazenamento;
use crate::config::{Config, ConfigS3, hash_senha};
use crate::{Compartilhado, Estado, aplicacao, banco, token};

const SENHA: &str = "senha-do-admin-de-teste";
const ORIGEM: &str = "http://painel.test";

fn estado(pool: PgPool, manter_backups: i64, tamanho_maximo: u64) -> Compartilhado {
    Arc::new(Estado {
        banco: pool,
        armazenamento: Armazenamento::memoria(),
        config: Config {
            endereco: "127.0.0.1:0".parse().unwrap(),
            banco_url: String::new(),
            s3: ConfigS3 {
                endpoint: String::new(),
                regiao: String::new(),
                bucket: String::new(),
                chave: String::new(),
                segredo: String::new(),
            },
            admin_usuario: "admin".into(),
            admin_senha_hash: hash_senha(SENHA).unwrap(),
            manter_backups,
            tamanho_maximo,
            url_publica: Some(ORIGEM.into()),
            cookie_seguro: true,
        },
        tentativas: Mutex::default(),
    })
}

async fn chamar(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let resposta = app.clone().oneshot(req).await.unwrap();
    let status = resposta.status();
    let cabecalhos = resposta.headers().clone();
    let corpo = resposta.into_body().collect().await.unwrap().to_bytes();
    (status, cabecalhos, corpo)
}

fn get(caminho: &str, token: &str) -> Request<Body> {
    Request::get(caminho)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn envio(token: &str, conteudo: &'static [u8], sha: &str) -> Request<Body> {
    Request::post("/api/v1/backups")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_LENGTH, conteudo.len())
        .header("x-sha256", sha)
        .header("x-nome-arquivo", "emissor-20261027-101500.nfbk")
        .body(Body::from(conteudo))
        .unwrap()
}

fn sha(conteudo: &[u8]) -> String {
    hex::encode(Sha256::digest(conteudo))
}

async fn cliente_com_token(pool: &PgPool, nome: &str) -> (i64, String) {
    let token = token::gerar();
    let dados = banco::DadosCliente {
        nome: nome.into(),
        ..Default::default()
    };
    let id = banco::criar_cliente(pool, &dados, &token::hash(&token)).await.unwrap();
    (id, token)
}

fn json(corpo: &Bytes) -> serde_json::Value {
    serde_json::from_slice(corpo).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn api_exige_token_valido(pool: PgPool) {
    let app = aplicacao(estado(pool.clone(), 30, 1 << 20));
    let sem = Request::get("/api/v1/situacao").body(Body::empty()).unwrap();
    assert_eq!(chamar(&app, sem).await.0, StatusCode::UNAUTHORIZED);
    let (status, _, corpo) = chamar(&app, get("/api/v1/situacao", &token::gerar())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(json(&corpo)["erro"].as_str().unwrap().contains("token inválido"));

    let (_, token) = cliente_com_token(&pool, "Escritório A").await;
    let (status, _, corpo) = chamar(&app, get("/api/v1/situacao", &token)).await;
    assert_eq!(status, StatusCode::OK);
    let s = json(&corpo);
    assert_eq!(
        (s["cliente"].as_str(), s["ativo"].as_bool()),
        (Some("Escritório A"), Some(true))
    );
    assert_eq!(s["mantidos"], 30);
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn envia_lista_e_baixa(pool: PgPool) {
    let app = aplicacao(estado(pool.clone(), 30, 1 << 20));
    let (_, token) = cliente_com_token(&pool, "A").await;
    let conteudo: &[u8] = b"conteudo cifrado do backup";
    let (status, _, corpo) = chamar(&app, envio(&token, conteudo, &sha(conteudo))).await;
    assert_eq!(status, StatusCode::CREATED);
    let novo = json(&corpo);
    assert_eq!(novo["nome"], "emissor-20261027-101500.nfbk");
    assert_eq!(novo["tamanho"], conteudo.len());

    let (_, _, corpo) = chamar(&app, get("/api/v1/backups", &token)).await;
    let lista = json(&corpo);
    assert_eq!(lista.as_array().unwrap().len(), 1);
    assert_eq!(lista[0]["sha256"], sha(conteudo));

    let id = novo["id"].as_i64().unwrap();
    let (status, cabecalhos, corpo) = chamar(&app, get(&format!("/api/v1/backups/{id}"), &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&corpo[..], conteudo);
    assert_eq!(cabecalhos["x-sha256"], sha(conteudo).as_str());

    // Backup de outro cliente não aparece nem baixa.
    let (_, outro) = cliente_com_token(&pool, "B").await;
    let (status, _, _) = chamar(&app, get(&format!("/api/v1/backups/{id}"), &outro)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn sha_diferente_nao_guarda_nada(pool: PgPool) {
    let estado = estado(pool.clone(), 30, 1 << 20);
    let app = aplicacao(estado.clone());
    let (_, token) = cliente_com_token(&pool, "A").await;
    let (status, _, _) = chamar(&app, envio(&token, b"conteudo", &sha(b"outro"))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(estado.armazenamento.quantos(), 0);
    let (_, _, corpo) = chamar(&app, get("/api/v1/backups", &token)).await;
    assert!(json(&corpo).as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn limite_de_tamanho(pool: PgPool) {
    let app = aplicacao(estado(pool.clone(), 30, 4));
    let (_, token) = cliente_com_token(&pool, "A").await;
    let (status, _, corpo) = chamar(&app, envio(&token, b"12345", &sha(b"12345"))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(json(&corpo)["erro"].as_str().unwrap().contains("limite"));
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn inativo_nao_envia_mas_restaura(pool: PgPool) {
    let app = aplicacao(estado(pool.clone(), 30, 1 << 20));
    let (id, token) = cliente_com_token(&pool, "A").await;
    let (_, _, corpo) = chamar(&app, envio(&token, b"antes", &sha(b"antes"))).await;
    let backup = json(&corpo)["id"].as_i64().unwrap();

    banco::definir_ativo(&pool, id, false).await.unwrap();
    let (status, _, corpo) = chamar(&app, envio(&token, b"depois", &sha(b"depois"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(json(&corpo)["erro"].as_str().unwrap().contains("Assinatura inativa"));

    let (status, _, corpo) = chamar(&app, get("/api/v1/situacao", &token)).await;
    assert_eq!((status, json(&corpo)["ativo"].as_bool()), (StatusCode::OK, Some(false)));
    let (status, _, corpo) = chamar(&app, get(&format!("/api/v1/backups/{backup}"), &token)).await;
    assert_eq!((status, &corpo[..]), (StatusCode::OK, &b"antes"[..]));
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn retencao_apaga_os_mais_antigos(pool: PgPool) {
    let estado = estado(pool.clone(), 2, 1 << 20);
    let app = aplicacao(estado.clone());
    let (_, token) = cliente_com_token(&pool, "A").await;
    for conteudo in [&b"um"[..], b"dois", b"tres"] {
        let req = Request::post("/api/v1/backups")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_LENGTH, conteudo.len())
            .header("x-sha256", sha(conteudo))
            .body(Body::from(conteudo))
            .unwrap();
        assert_eq!(chamar(&app, req).await.0, StatusCode::CREATED);
    }
    let (_, _, corpo) = chamar(&app, get("/api/v1/backups", &token)).await;
    let lista = json(&corpo);
    let shas: Vec<_> = lista.as_array().unwrap().iter().map(|b| b["sha256"].clone()).collect();
    assert_eq!(shas, [sha(b"tres"), sha(b"dois")]);
    assert_eq!(estado.armazenamento.quantos(), 2);
}

fn formulario(caminho: &str, corpo: &str, origem: Option<&str>, cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::post(caminho).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(o) = origem {
        req = req.header(header::ORIGIN, o);
    }
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    req.body(Body::from(corpo.to_owned())).unwrap()
}

async fn entrar(app: &Router) -> String {
    let (status, cabecalhos, _) = chamar(
        app,
        formulario("/login", &format!("usuario=admin&senha={SENHA}"), Some(ORIGEM), None),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let cookie = cabecalhos[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Strict"));
    cookie.split(';').next().unwrap().to_owned()
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn painel_exige_login_e_mesma_origem(pool: PgPool) {
    let app = aplicacao(estado(pool, 30, 1 << 20));
    let (status, cabecalhos, _) = chamar(&app, Request::get("/").body(Body::empty()).unwrap()).await;
    assert_eq!(
        (status, cabecalhos[header::LOCATION].to_str().unwrap()),
        (StatusCode::SEE_OTHER, "/login")
    );

    let senha_certa = format!("usuario=admin&senha={SENHA}");
    let (status, _, _) = chamar(&app, formulario("/login", &senha_certa, None, None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = chamar(
        &app,
        formulario("/login", &senha_certa, Some("https://outro.test"), None),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let cookie = entrar(&app).await;
    let req = Request::get("/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let (status, _, corpo) = chamar(&app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&corpo).contains("Nenhum cliente ainda"));

    // Sair invalida a sessão.
    chamar(&app, formulario("/sair", "", Some(ORIGEM), Some(&cookie))).await;
    let req = Request::get("/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    assert_eq!(chamar(&app, req).await.0, StatusCode::SEE_OTHER);
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn login_bloqueia_depois_de_tentativas(pool: PgPool) {
    let app = aplicacao(estado(pool, 30, 1 << 20));
    for _ in 0..5 {
        let (status, _, _) = chamar(
            &app,
            formulario("/login", "usuario=admin&senha=errada", Some(ORIGEM), None),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let certa = format!("usuario=admin&senha={SENHA}");
    let (status, _, _) = chamar(&app, formulario("/login", &certa, Some(ORIGEM), None)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requer Postgres (DATABASE_URL)"]
async fn cliente_criado_no_painel_usa_o_token(pool: PgPool) {
    let app = aplicacao(estado(pool, 30, 1 << 20));
    let cookie = entrar(&app).await;
    let (status, _, corpo) = chamar(
        &app,
        formulario(
            "/clientes/novo",
            "nome=Escrit%C3%B3rio+B&documento=&email=&observacao=",
            Some(ORIGEM),
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let html = String::from_utf8_lossy(&corpo).into_owned();
    let inicio = html.find(token::PREFIXO).expect("token na página");
    let token: String = html[inicio..].chars().take_while(|c| *c != '"').collect();
    let (status, _, corpo) = chamar(&app, get("/api/v1/situacao", &token)).await;
    assert_eq!(
        (status, json(&corpo)["cliente"].as_str()),
        (StatusCode::OK, Some("Escritório B"))
    );

    // Novo token: o anterior para de valer.
    let id = 1;
    let (status, _, _) = chamar(
        &app,
        formulario(&format!("/clientes/{id}/token"), "", Some(ORIGEM), Some(&cookie)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        chamar(&app, get("/api/v1/situacao", &token)).await.0,
        StatusCode::UNAUTHORIZED
    );
}
