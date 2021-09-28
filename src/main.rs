use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, Result};
mod api;
mod params;
use api::Douban;
use params::Search;
use structopt::StructOpt;

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
async fn movies(query: web::Query<Search>) -> Result<String> {
    if query.q.is_empty() {
        return Ok("[]".to_string());
    }

    if query.search_type.as_ref().unwrap_or(&String::new()) == "full" {
        let result = Douban::new().search_full(&query.q).await.unwrap();
        return Ok(serde_json::to_string(&result).unwrap());
    } else {
        let result = Douban::new().search(&query.q).await.unwrap();
        return Ok(serde_json::to_string(&result).unwrap());
    }
}

/// {sid} - deserializes to a String
#[get("/movies/{sid}")]
async fn movie(path: web::Path<String>) -> Result<String> {
    let sid = path.into_inner();
    let result = Douban::new().get_movie_info(&sid).await.unwrap();
    return Ok(serde_json::to_string(&result).unwrap());
}

#[get("/movies/{sid}/celebrities")]
async fn celebrities(path: web::Path<String>) -> Result<String> {
    let sid = path.into_inner();
    let result = Douban::new().get_movie_info(&sid).await.unwrap();
    return Ok(serde_json::to_string(&result.celebrities).unwrap());
}

#[get("/celebrities/{id}")]
async fn celebrity(path: web::Path<String>) -> Result<String> {
    let id = path.into_inner();
    let result = Douban::new().get_celebrity(&id).await.unwrap();
    return Ok(serde_json::to_string(&result).unwrap());
}

#[get("/photo/{sid}")]
async fn photo(path: web::Path<String>) -> Result<String> {
    let sid = path.into_inner();
    let result = Douban::new().get_wallpaper(&sid).await.unwrap();
    return Ok(serde_json::to_string(&result).unwrap());
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();

    println!("listening on {}:{:?}", opt.host, opt.port);
    HttpServer::new(|| {
        App::new()
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
