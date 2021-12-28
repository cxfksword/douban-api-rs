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
    re_binding: Regex,       //装帧 正则
    // re_category: Regex,      //分类 正则
    re_isbn: Regex,      //isbn 正则
    re_pages: Regex,     //页数 正则
    re_price: Regex,     //价格 正则
    re_pubdate: Regex,   //出版时间 正则
    re_publisher: Regex, //出版社 正则
    re_subtitle: Regex,  //副标题 正则
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
        let re_binding = Regex::new(r"装帧: (.+?)\n").unwrap();
        // let re_category = Regex::new(r"分类: (.+?)\n").unwrap();
        let re_isbn = Regex::new(r"ISBN: (.+?)\n").unwrap();
        let re_pages = Regex::new(r"页数: (.+?)\n").unwrap();
        let re_price = Regex::new(r"定价: (.+?)\n").unwrap();
        let re_pubdate = Regex::new(r"出版年: (.+?)\n").unwrap();
        let re_publisher = Regex::new(r"出版社: (.+?)\n").unwrap();
        let re_subtitle = Regex::new(r"副标题: (.+?)\n").unwrap();
        Self {
            client,
            re_binding,
            // re_category,
            re_isbn,
            re_pages,
            re_price,
            re_pubdate,
            re_publisher,
            re_subtitle,
        }
    }

    pub async fn search(&self, q: &str, count: i32) -> Result<DoubanBookResult> {
        let ids = self.get_ids(q, count).await.unwrap();
        let mut list = Vec::with_capacity(ids.len());
        for i in ids {
            if !i.title.contains(q) {
                continue;
            }
            match self.get_book_info(&i.id).await {
                Ok(info) => list.push(info),
                Err(_e) => {}
            }
        }
        Ok(DoubanBookResult {
            code: 0,
            books: list,
            msg: "".to_string(),
        })
    }

    async fn get_ids(&self, q: &str, count: i32) -> Result<Vec<BookListItem>> {
        let mut vec = Vec::with_capacity(count as usize);
        if q.is_empty() {
            return Ok(vec);
        }

        let url = "https://m.douban.com/j/search/";
        let res = self
            .client
            .get(url)
            .query(&[("q", q), ("t", "book")])
            .send()
            .await?
            .error_for_status();
        match res {
            Ok(res) => {
                let res = res.json::<HtmlResult>().await?;
                let document = Vis::load(&res.html).unwrap();
                vec = document
                    .find("li")
                    .map(|_index, x| {
                        let dom = Vis::dom(x);
                        let title = dom.find("a div span").first().text().to_string();
                        let href = dom.find("a").attr("href").unwrap().to_string();
                        let t_array = href.split("/").collect::<Vec<&str>>();
                        let id = t_array[t_array.len() - 2].to_string();
                        BookListItem {
                            title: title,
                            id: id,
                        }
                    })
                    .into_iter()
                    .take(count as usize)
                    .collect::<Vec<BookListItem>>();
            }
            Err(err) => {
                println!("错误: {:?}", err);
            }
        }

        Ok(vec)
    }

    async fn get_book_internal(&self, url: String) -> Result<DoubanBook> {
        let res = self.client.get(url).send().await?.error_for_status();
        let result_text: String;
        match res {
            Err(e) => {
                println!("{}", e);
                return Err(anyhow::Error::from(e));
            }
            Ok(t) => result_text = (t.text().await?).clone(),
        }

        let document = Vis::load(&result_text).unwrap();
        let x = document.find("#wrapper");
        let id = document
            .find("//meta[@property='og:url']/content[0]")
            .text()
            .to_string();
        let title = x.find("h1>span:first-child").text().to_string();
        let large_img = x.find("a.nbg").attr("href").unwrap().to_string();
        let small_img = x.find("a.nbg>img").attr("src").unwrap().to_string();
        let content = x.find("#content");
        let mut tags = Vec::default();
        x.find("a.tag").map(|_index, t| {
            tags.push(Tag {
                name: t.text().to_string(),
            });
        });

        let rating_str = content
            .find("div.rating_self strong.rating_num")
            .text()
            .trim()
            .to_string();
        let rating = if rating_str.is_empty() {
            Rating { average: 0.0 }
        } else {
            Rating {
                average: rating_str.parse::<f32>().unwrap(),
            }
        };
        let summary = content
            .find("#link-report :not(.short) .intro")
            .text()
            .trim()
            .replace("©豆瓣", "")
            .to_string();
        let author_intro = content
            .find("div.related_info .indent .intro")
            .text()
            .trim()
            .to_string();
        let info = content.find("#info");
        let mut author = Vec::with_capacity(1);
        let mut translators = Vec::with_capacity(1);
        let mut producer = String::new();
        let mut serials = String::new();
        info.find("span.pl").map(|_index, x| {
            match x.text().trim().to_string().as_str() {
                "作者" => self.get_texts(x, &mut author),
                "译者" => self.get_texts(x, &mut translators),
                "出品方:" => producer = self.get_text(x),
                "丛书:" => serials = self.get_text(x),
                _ => {}
            };
        });
        let category = String::from(""); //TODO 页面上是在找不到分类...
        let info_text = info.text().to_string();
        let (publisher, pubdate, pages, price, binding, subtitle, isbn13) =
            self.parse_info(&info_text);

        let images = Image {
            medium: "".to_string(),
            large: large_img,
            small: small_img,
        };
        let cache_key = id.clone();
        let info = DoubanBook {
            id,
            author,
            author_intro,
            translators,
            images,
            binding,
            category,
            rating,
            isbn13,
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

    pub async fn get_book_info_by_isbn(&self, isbn: &str) -> Result<DoubanBook> {
        let url = format!("https://douban.com/isbn/{}/", isbn);
        self.get_book_internal(url).await
    }

    pub async fn get_book_info(&self, id: &String) -> Result<DoubanBook> {
        if BOOK_CACHE.get(id).is_some() {
            return Ok(BOOK_CACHE.get(id).unwrap());
        }
        let url = format!("https://book.douban.com/subject/{}/", id);
        self.get_book_internal(url).await
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

    fn parse_info(&self, text: &str) -> (String, String, String, String, String, String, String) {
        let publisher = self.get_regex_text(text, &self.re_publisher);
        let pubdate = self.get_regex_text(text, &self.re_pubdate);
        let pages = self.get_regex_text(text, &self.re_pages);
        let price = self.get_regex_text(text, &self.re_price);
        let binding = self.get_regex_text(text, &self.re_binding);
        let subtitle = self.get_regex_text(text, &self.re_subtitle);
        let isbn13 = self.get_regex_text(text, &self.re_isbn);

        (publisher, pubdate, pages, price, binding, subtitle, isbn13)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubanBookResult {
    code: u32,
    msg: String,
    books: Vec<DoubanBook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubanBook {
    id: String,               //id
    author: Vec<String>,      //作者
    author_intro: String,     //作者简介
    translators: Vec<String>, //译者
    images: Image,            //封面
    binding: String,          //装帧方式
    category: String,         //分类
    rating: Rating,           //评分
    isbn13: String,           //isbn
    pages: String,            //页数
    price: String,            //价格
    pubdate: String,          //出版时间
    publisher: String,        //出版社
    producer: String,         //出品方
    serials: String,          //丛书
    subtitle: String,         //副标题
    summary: String,          //简介
    title: String,            //书名
    tags: Vec<Tag>,           //标签
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    small: String,
    medium: String,
    large: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    average: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HtmlResult {
    count: i32,
    html: String,
    limit: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookListItem {
    title: String,
    id: String,
}
