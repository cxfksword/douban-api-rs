use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder, Result};
mod api;
mod bookapi;
use api::Douban;
use bookapi::DoubanBookApi;
use serde::Deserialize;
use std::env;
use structopt::StructOpt;

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            r#"
       接口列表：<br/>
       /movies?q={movie_name}<br/>
       /movies?q={movie_name}&type=full<br/>
       /movies/{sid}<br/>
       /movies/{sid}/celebrities<br/>
       /celebrities/{cid}<br/>
       /photo/{sid}<br/>
       /v2/book/search?q={book_name}<br/>
       /v2/book/{sid}<br/>
       /v2/book/isbn/{isbn}<br/>
    "#,
        )
}

#[get("/movies")]
async fn movies(
    query: web::Query<Search>,
    douban_api: web::Data<Douban>,
    opt: web::Data<Opt>,
) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }

    let count = query.count.unwrap_or(0);
    if query.search_type.as_ref().unwrap_or(&String::new()) == "full" {
        let result = douban_api.search_full(&query.q, count).await.unwrap();
        Ok(serde_json::to_string(&result).unwrap())
    } else {
        let result = douban_api
            .search(&query.q, count, &opt.proxy)
            .await
            .unwrap();
        Ok(serde_json::to_string(&result).unwrap())
    }
}

/// {sid} - deserializes to a String
#[get("/movies/{sid}")]
async fn movie(path: web::Path<String>, douban_api: web::Data<Douban>) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api.get_movie_info(&sid).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/movies/{sid}/celebrities")]
async fn celebrities(path: web::Path<String>, douban_api: web::Data<Douban>) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api.get_celebrities(&sid).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/celebrities/{id}")]
async fn celebrity(path: web::Path<String>, douban_api: web::Data<Douban>) -> Result<String> {
    let id = path.into_inner();
    let result = douban_api.get_celebrity(&id).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/photo/{sid}")]
async fn photo(path: web::Path<String>, douban_api: web::Data<Douban>) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api.get_wallpaper(&sid).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/v2/book/search")]
async fn books(query: web::Query<Search>, book_api: web::Data<DoubanBookApi>) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }
    let count = query.count.unwrap_or(2);
    let result = book_api.search(&query.q, count).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/v2/book/{sid}")]
async fn book(path: web::Path<String>, book_api: web::Data<DoubanBookApi>) -> Result<String> {
    let sid = path.into_inner();
    match book_api.get_book_info(&sid).await {
        Ok(info) => Ok(serde_json::to_string(&info).unwrap()),
        Err(e) => Err(actix_web::error::ErrorInternalServerError(e)),
    }
}

#[get("/v2/book/isbn/{isbn}")]
async fn book_by_isbn(
    path: web::Path<String>,
    book_api: web::Data<DoubanBookApi>,
) -> Result<String> {
    let isbn = path.into_inner();
    match book_api.get_book_info_by_isbn(&isbn).await {
        Ok(info) => Ok(serde_json::to_string(&info).unwrap()),
        Err(e) => Err(actix_web::error::ErrorInternalServerError(e)),
    }
}

#[get("/proxy")]
async fn proxy(query: web::Query<Proxy>, douban_api: web::Data<Douban>) -> impl Responder {
    let resp = douban_api.proxy_img(&query.url).await.unwrap();
    let content_type = resp.headers().get("content-type").unwrap();
    HttpResponse::build(resp.status())
        .append_header(("content-type", content_type))
        .body(resp.bytes().await.unwrap())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=info,actix_server=info");
    env_logger::init();
    let opt = Opt::from_args();
    let douban = Douban::new(opt.limit);

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::new("%a \"%r\" %s %b %T"))
            .app_data(web::Data::new(douban.clone()))
            .app_data(web::Data::new(DoubanBookApi::new()))
            .app_data(web::Data::new(Opt::from_args()))
            .service(index)
            .service(movies)
            .service(movie)
            .service(celebrities)
            .service(celebrity)
            .service(photo)
            .service(book)
            .service(books)
            .service(book_by_isbn)
            .service(proxy)
    })
    .bind((opt.host, opt.port))?
    .run()
    .await
}

#[derive(StructOpt, Debug)]
#[structopt(name = "douban-api-rs")]
struct Opt {
    /// Listen host
    #[structopt(long, default_value = "0.0.0.0")]
    host: String,
    /// Listen port
    #[structopt(short, long, default_value = "8080")]
    port: u16,
    #[structopt(short = "I", long, default_value = "")]
    proxy: String,
    #[structopt(long, default_value = "3")]
    limit: usize,
}

#[derive(Deserialize)]
struct Search {
    pub q: String,
    #[serde(alias = "type")]
    pub search_type: Option<String>,
    pub count: Option<i32>,
}

#[derive(Deserialize)]
struct Proxy {
    pub url: String,
}
