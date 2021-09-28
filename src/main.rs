use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder, Result};
mod api;
use api::Douban;
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
    "#,
        )
}

#[get("/movies")]
async fn movies(query: web::Query<Search>, douban_api: web::Data<Douban>) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }

    let count = query.count.unwrap_or(0);
    if query.search_type.as_ref().unwrap_or(&String::new()) == "full" {
        let result = douban_api.search_full(&query.q, count).await.unwrap();
        Ok(serde_json::to_string(&result).unwrap())
    } else {
        let result = douban_api.search(&query.q, count).await.unwrap();
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
    let result = douban_api.get_movie_info(&sid).await.unwrap();
    Ok(serde_json::to_string(&result.celebrities).unwrap())
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();
    let opt = Opt::from_args();

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::new("%a \"%r\" %s %b %T"))
            .app_data(web::Data::new(Douban::new()))
            .service(index)
            .service(movies)
            .service(movie)
            .service(celebrities)
            .service(celebrity)
            .service(photo)
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
}

#[derive(Deserialize)]
struct Search {
    pub q: String,
    #[serde(alias = "type")]
    pub search_type: Option<String>,
    pub count: Option<i32>,
}
