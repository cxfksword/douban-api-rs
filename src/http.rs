use crate::config::Opt;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{cookie::Jar, Error, IntoUrl, Request, RequestBuilder, Response, Url};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

const ORIGIN: &str = "https://movie.douban.com";
const REFERER: &str = "https://movie.douban.com/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client, //请求客户端
}

impl HttpClient {
    pub fn new(config: Opt) -> HttpClient {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));

        let url = "https://douban.com/".parse::<Url>().unwrap();
        let jar = Jar::default();
        if !config.cookie.is_empty() {
            for s in config.cookie.split(";") {
                let cookie_str = format!("{}; Domain=douban.com", s);
                jar.add_cookie_str(cookie_str.as_str(), &url);
            }
            println!("{:?}", jar);
        }
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            .cookie_provider(Arc::new(jar))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            // .connection_verbose(true)
            .build()
            .unwrap();
        Self { client }
    }

    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.client.get(url)
    }

    #[allow(dead_code)]
    pub fn execute(&self, request: Request) -> impl Future<Output = Result<Response, Error>> {
        self.client.execute(request)
    }
}
