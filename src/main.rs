use actix_web::{
    get, middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
mod api;
mod bookapi;
use api::Douban;
use bookapi::DoubanBookApi;
use clap::Parser;
use serde::Deserialize;
use std::env;

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
       /v2/book/id/{sid}<br/>
       /v2/book/isbn/{isbn}<br/>
    "#,
        )
}

#[get("/movies")]
async fn movies(
    douban_api: web::Data<Douban>,
    req: HttpRequest,
    query: web::Query<SearchQuery>,
    opt: web::Data<Opt>,
) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }

    // 没有useragent或为空，是来自jellyfin-plugin-opendouban插件的请求
    let from_jellyfin = !req.headers().contains_key("User-Agent")
        || req
            .headers()
            .get("User-Agent")
            .unwrap()
            .to_str()
            .unwrap()
            .is_empty();

    let mut count = query.count.unwrap_or(0);
    if count == 0 && from_jellyfin {
        count = opt.limit as i32
    }

    if query.search_type == "full" {
        let result = douban_api
            .search_full(&query.q, count, &query.image_size)
            .await
            .unwrap();
        Ok(serde_json::to_string(&result).unwrap())
    } else {
        let result = douban_api
            .search(&query.q, count, &query.image_size)
            .await
            .unwrap();
        Ok(serde_json::to_string(&result).unwrap())
    }
}

/// {sid} - deserializes to a String
#[get("/movies/{sid}")]
async fn movie(
    douban_api: web::Data<Douban>,
    path: web::Path<String>,
    query: web::Query<MovieQuery>,
) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api
        .get_movie_info(&sid, &query.image_size)
        .await
        .unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/movies/{sid}/celebrities")]
async fn celebrities(douban_api: web::Data<Douban>, path: web::Path<String>) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api.get_celebrities(&sid).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/celebrities/{id}")]
async fn celebrity(douban_api: web::Data<Douban>, path: web::Path<String>) -> Result<String> {
    let id = path.into_inner();
    let result = douban_api.get_celebrity(&id).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/photo/{sid}")]
async fn photo(douban_api: web::Data<Douban>, path: web::Path<String>) -> Result<String> {
    let sid = path.into_inner();
    let result = douban_api.get_wallpaper(&sid).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/v2/book/search")]
async fn books(
    query: web::Query<SearchQuery>,
    book_api: web::Data<DoubanBookApi>,
) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }
    let count = query.count.unwrap_or(2);
    if count > 20 {
        return Err(actix_web::error::ErrorBadRequest(
            "{\"message\":\"count不能大于20\"}",
        ));
    }
    let result = book_api.search(&query.q, count).await.unwrap();
    Ok(serde_json::to_string(&result).unwrap())
}

#[get("/v2/book/id/{sid}")]
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
async fn proxy(query: web::Query<ProxyQuery>, douban_api: web::Data<Douban>) -> impl Responder {
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
    let opt = Opt::parse();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(Douban::new()))
            .app_data(web::Data::new(DoubanBookApi::new()))
            .app_data(web::Data::new(Opt::parse()))
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

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Opt {
    /// Listen host
    #[clap(long, default_value = "0.0.0.0")]
    host: String,
    /// Listen port
    #[clap(short, long, default_value = "8080")]
    port: u16,
    #[clap(short, long, default_value = "3", env = "DOUBAN_API_LIMIT_SIZE")]
    limit: usize,
}

#[derive(Deserialize)]
struct SearchQuery {
    pub q: String,
    #[serde(alias = "type", default)]
    pub search_type: String,
    #[serde(alias = "s", default)]
    pub image_size: String,
    pub count: Option<i32>,
}

#[derive(Deserialize)]
struct MovieQuery {
    #[serde(alias = "s", default)]
    pub image_size: String,
}

#[derive(Deserialize)]
struct ProxyQuery {
    pub url: String,
}
