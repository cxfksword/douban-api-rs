use anyhow::Result;
use lazy_static::*;
use moka::future::{Cache, CacheBuilder};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use visdom::types::BoxDynElement;
use visdom::Vis;

lazy_static! {
    static ref BOOK_CACHE: Cache<String, DoubanBook> = CacheBuilder::new(CACHE_SIZE)
        .time_to_live(Duration::from_secs(10 * 60))
        .build();
}

const ORIGIN: &str = "https://book.douban.com";
const REFERER: &str = "https://book.douban.com/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";
const CACHE_SIZE: usize = 100;

#[derive(Clone)]
pub struct DoubanBookApi {
    client: reqwest::Client, //请求客户端
    re_id: Regex,            //id 正则
    re_binding: Regex,       //装帧 正则
    re_category: Regex,      //分类 正则
    re_isbn: Regex,          //isbn 正则
    re_pages: Regex,         //页数 正则
    re_price: Regex,         //价格 正则
    re_pubdate: Regex,       //出版时间 正则
    re_publisher: Regex,     //出版社 正则
    re_subtitle: Regex,      //副标题 正则
}

impl DoubanBookApi {
    pub fn new() -> DoubanBookApi {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let re_id = Regex::new(r"sid: (\d+?),").unwrap();
        let re_binding = Regex::new(r"装帧: (.+?)\n").unwrap();
        let re_category = Regex::new(r"分类: (.+?)\n").unwrap();
        let re_isbn = Regex::new(r"ISBN: (.+?)\n").unwrap();
        let re_pages = Regex::new(r"页数: (.+?)\n").unwrap();
        let re_price = Regex::new(r"定价: (.+?)\n").unwrap();
        let re_pubdate = Regex::new(r"出版年: (.+?)\n").unwrap();
        let re_publisher = Regex::new(r"出版社: (.+?)\n").unwrap();
        let re_subtitle = Regex::new(r"副标题: (.+?)\n").unwrap();
        Self {
            client,
            re_id,
            re_binding,
            re_category,
            re_isbn,
            re_pages,
            re_price,
            re_pubdate,
            re_publisher,
            re_subtitle,
        }
    }

    pub async fn search(&self, q: &str, count: i32) -> Result<Vec<DoubanBook>> {
        let ids = self.get_ids(q, count).await.unwrap();
        let mut list = Vec::with_capacity(ids.len());
        for i in ids {
            match self.get_book_info(&i).await {
                Ok(info) => list.push(info),
                Err(_e) => {}
            }
        }
        Ok(list)
    }

    async fn get_ids(&self, q: &str, count: i32) -> Result<Vec<String>> {
        let mut vec = Vec::with_capacity(count as usize);
        if q.is_empty() {
            return Ok(vec);
        }

        let url = "https://www.douban.com/search";
        let res = self
            .client
            .get(url)
            .query(&[("q", q), ("cat", "1001")])
            .send()
            .await?
            .error_for_status();
        match res {
            Ok(res) => {
                let res = res.text().await?;
                let document = Vis::load(&res).unwrap();
                vec = document
                    .find("div.result-list")
                    .first()
                    .find(".result")
                    .map(|_index, x| {
                        let x = Vis::dom(x);
                        let onclick = x.find("div.title a").attr("onclick").unwrap().to_string();
                        let id = self.parse_id(&onclick);
                        id
                    })
                    .into_iter()
                    .take(count as usize)
                    .collect::<Vec<String>>();
            }
            Err(err) => {
                println!("错误: {:?}", err);
            }
        }

        Ok(vec)
    }

    pub async fn get_book_info(&self, id: &str) -> Result<DoubanBook> {
        let cache_key = id.to_string();
        if BOOK_CACHE.get(&cache_key).is_some() {
            return Ok(BOOK_CACHE.get(&cache_key).unwrap());
        }
        let url = format!("https://book.douban.com/subject/{}/", id);
        let res = self.client.get(url).send().await?.error_for_status();
        let result_text: String;
        match res {
            Err(e) => {
                println!("{}", e);
                return Err(anyhow::Error::from(e));
            }
            Ok(t) => result_text = (t.text().await?).clone(),
        }

        // let res = res.unwrap().text().await?;
        let document = Vis::load(&result_text).unwrap();
        let x = document.find("#wrapper");
        let id = id.to_string();
        let title = x.find("h1>span:first-child").text().to_string();
        let large_img = x.find("a.nbg").attr("href").unwrap().to_string();
        let small_img = x.find("a.nbg>img").attr("src").unwrap().to_string();
        let content = x.find("#content");
        let mut tags = Vec::default();
        x.find("a.tag").map(|_index, t| {
            tags.push(t.text().to_string());
        });
        let rating = content
            .find("div.rating_self strong.rating_num")
            .text()
            .trim()
            .to_string();
        let summary = content
            .find("#link-report :not(.short) .intro")
            .text()
            .trim()
            .replace("©豆瓣", "")
            .to_string();
        let info = content.find("#info");
        let mut authors = Vec::with_capacity(1);
        let mut translators = Vec::with_capacity(1);
        let mut producer = String::new();
        let mut serials = String::new();
        info.find("span.pl").map(|_index, x| {
            match x.text().trim().to_string().as_str() {
                "作者" => self.get_texts(x, &mut authors),
                "译者" => self.get_texts(x, &mut translators),
                "出品方:" => producer = self.get_text(x),
                "丛书:" => serials = self.get_text(x),
                _ => {}
            };
        });
        let category = String::from(""); //TODO 页面上是在找不到分类...
        let info_text = info.text().to_string();
        let (publisher, pubdate, pages, price, binding, subtitle, isbn) =
            self.parse_info(&info_text);

        let images = Image {
            medium: "".to_string(),
            large: large_img,
            small: small_img,
        };
        let info = DoubanBook {
            id,
            authors,
            translators,
            images,
            binding,
            category,
            rating,
            isbn,
            pages,
            price,
            pubdate,
            publisher,
            producer,
            serials,
            subtitle,
            summary,
            title,
            tags,
        };
        BOOK_CACHE.insert(cache_key, info.clone()).await;
        Ok(info)
    }

    fn get_text(&self, e: &BoxDynElement) -> String {
        match e.next_element_sibling() {
            Some(x) => x.text().to_string(),
            None => String::new(),
        }
    }

    fn get_texts(&self, e: &BoxDynElement, vec: &mut Vec<String>) {
        for e in e.next_element_siblings() {
            vec.push(e.text().to_string());
        }
    }

    fn get_regex_text(&self, s: &str, regex: &Regex) -> String {
        match regex.captures(s) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        }
    }

    fn parse_id(&self, text: &str) -> String {
        let mut id = String::new();
        for c in self.re_id.captures_iter(text) {
            id = c[1].to_string();
        }
        id
    }

    fn parse_info(&self, text: &str) -> (String, String, String, String, String, String, String) {
        let publisher = self.get_regex_text(text, &self.re_publisher);
        let pubdate = self.get_regex_text(text, &self.re_pubdate);
        let pages = self.get_regex_text(text, &self.re_pages);
        let price = self.get_regex_text(text, &self.re_price);
        let binding = self.get_regex_text(text, &self.re_binding);
        let subtitle = self.get_regex_text(text, &self.re_subtitle);
        let isbn = self.get_regex_text(text, &self.re_isbn);

        (publisher, pubdate, pages, price, binding, subtitle, isbn)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubanBook {
    id: String,               //id
    authors: Vec<String>,     //作者
    translators: Vec<String>, //译者
    images: Image,            //封面
    binding: String,          //装帧方式
    category: String,         //分类
    rating: String,           //评分
    isbn: String,             //isbn
    pages: String,            //页数
    price: String,            //价格
    pubdate: String,          //出版时间
    publisher: String,        //出版社
    producer: String,         //出品方
    serials: String,          //丛书
    subtitle: String,         //副标题
    summary: String,          //简介
    title: String,            //书名
    tags: Vec<String>,        //标签
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    small: String,
    medium: String,
    large: String,
}
