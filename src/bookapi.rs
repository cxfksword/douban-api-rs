use anyhow::Result;
use lazy_static::*;
use moka::future::{Cache, CacheBuilder};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::time::Duration;
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
        Self { client, re_id }
    }

    pub async fn search(&self, q: &str, count: i32) -> Result<DoubanBookResult<DoubanBook>> {
        let list = self.get_list(q, count).await.unwrap();
        Ok(DoubanBookResult {
            code: 0,
            books: list,
            msg: "".to_string(),
        })
    }

    async fn get_list(&self, q: &str, count: i32) -> Result<Vec<DoubanBook>> {
        let mut vec = Vec::with_capacity(count as usize);
        if q.is_empty() {
            return Ok(vec);
        }
        let url = "https://www.douban.com/search";
        let res = self
            .client
            .get(url)
            .query(&[("cat", "1001"), ("q", q)])
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
                        let title = x.find("div.title a").text().trim().to_string();
                        let summary = x.find("p").text().trim().to_string();
                        let large = x.find(".pic img").attr("src").unwrap().to_string();
                        let rate = x.find(".rating_nums").text().to_string();
                        let sub_str = x.find(".subject-cast").text().to_string();
                        let subjects: Vec<&str> = sub_str.split('/').collect();
                        let len = subjects.len();
                        let mut pubdate = String::from("");
                        let mut publisher = String::from("");
                        let mut author = Vec::new();
                        if len >= 3 {
                            pubdate = subjects[len - 1].trim().to_string();
                            publisher = subjects[len - 2].trim().to_string();
                            let mut i = 0;
                            for elem in subjects {
                                author.push(elem.trim().to_string());
                                i += 1;
                                if i == len - 2 {
                                    break;
                                }
                            }
                        } else if len == 2 {
                            author.push(subjects[0].trim().to_string());
                            match subjects[1].parse::<i32>() {
                                Ok(_t) => pubdate = subjects[1].trim().to_string(),
                                Err(_e) => publisher = subjects[1].trim().to_string(),
                            }
                        } else if len == 1 {
                            author.push(subjects[0].trim().to_string());
                        }

                        let mut m_id = String::from("");
                        for c in self.re_id.captures_iter(&onclick) {
                            m_id = c[1].trim().to_string();
                        }
                        let id = m_id;

                        let rating = if rate.is_empty() {
                            Rating::new(0.0)
                        } else {
                            Rating::new(rate.parse::<f32>().unwrap())
                        };
                        let images = Image::new(large);
                        DoubanBook::simple(SimpleDoubanBook {
                            id,
                            author,
                            images,
                            rating,
                            pubdate,
                            publisher,
                            summary,
                            title,
                        })
                    })
                    .into_iter()
                    .take(count as usize)
                    .collect::<Vec<DoubanBook>>();
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
        let id: String;
        match res {
            Err(e) => {
                println!("{}", e);
                return Err(anyhow::Error::from(e));
            }
            Ok(t) => {
                let t_url = t.url().as_str();
                let t_array = t_url.split('/').collect::<Vec<&str>>();
                id = t_array[t_array.len() - 2].to_string();
                result_text = t.text().await?
            }
        }

        let document = Vis::load(&result_text).unwrap();
        let x = document.find("#wrapper");
        let title = x.find("h1>span:first-child").text().to_string();
        let large_img = x.find("a.nbg").attr("href").unwrap().to_string();
        let small_img = x.find("a.nbg>img").attr("src").unwrap().to_string();
        let content = x.find("#content");
        let mut tags = Vec::default();
        x.find("a.tag").map(|_index, t| {
            tags.push(Tag { name: t.text() });
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
            .find("#link-report .hidden .intro")
            .html()
            .replace("©豆瓣", "");
        let author_intro = content
            .find(".related_info > .indent:not(id) > .all .intro")
            .html()
            .trim()
            .to_string();

        let mut author = Vec::with_capacity(1);
        let mut translators = Vec::with_capacity(1);
        let mut producer = String::new();
        let mut serials = String::new();
        let mut origin = String::new();
        let mut publisher = String::new();
        let mut pubdate = String::new();
        let mut pages = String::new();
        let mut price = String::new();
        let mut binding = String::new();
        let mut subtitle = String::new();
        let mut isbn13 = String::new();
        let category = String::from(""); //TODO 页面上是在找不到分类...
        let info = content.find("#info");
        let info_array = info
            .text()
            .trim()
            .split('\n')
            .filter(|&x| !x.trim().is_empty())
            .map(|x| x.trim())
            .collect::<Vec<&str>>();
        let mut i = 0;
        loop {
            match info_array[i] {
                "作者:" => {
                    self.get_texts(&info_array[i + 1..], &mut author);
                    i += 1;
                    continue;
                }
                "译者:" => {
                    self.get_texts(&info_array[i + 1..], &mut translators);
                    i += 1;
                    continue;
                }
                _ => {}
            }

            if info_array[i].starts_with("出品方:") {
                producer = self.get_text(info_array[i])
            } else if info_array[i].starts_with("出版社:") {
                publisher = self.get_text(info_array[i])
            } else if info_array[i].starts_with("副标题:") {
                subtitle = self.get_text(info_array[i])
            } else if info_array[i].starts_with("原作名:") {
                origin = self.get_text(info_array[i])
            } else if info_array[i].starts_with("出版年:") {
                pubdate = self.get_text(info_array[i])
            } else if info_array[i].starts_with("页数:") {
                pages = self.get_text(info_array[i])
            } else if info_array[i].starts_with("定价:") {
                price = self.get_text(info_array[i])
            } else if info_array[i].starts_with("装帧:") {
                binding = self.get_text(info_array[i])
            } else if info_array[i].starts_with("丛书:") {
                serials = self.get_text(info_array[i])
            } else if info_array[i].starts_with("ISBN:") {
                isbn13 = self.get_text(info_array[i])
            }

            if i == info_array.len() - 1 {
                break;
            }
            i += 1;
        }
        let images = Image {
            medium: large_img.clone(),
            large: large_img,
            small: small_img,
        };
        let cache_key = id.clone();
        let cache_key1 = isbn13.clone();
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
            origin,
        };
        BOOK_CACHE.insert(cache_key, info.clone()).await;
        BOOK_CACHE.insert(cache_key1, info.clone()).await;
        Ok(info)
    }

    pub async fn get_book_info_by_isbn(&self, isbn: &str) -> Result<DoubanBook> {
        let cache_key = isbn.to_string();
        if BOOK_CACHE.get(&cache_key).is_some() {
            return Ok(BOOK_CACHE.get(&cache_key).unwrap());
        }

        let url = format!("https://douban.com/isbn/{}/", isbn);
        self.get_book_internal(url).await
    }

    pub async fn get_book_info(&self, id: &str) -> Result<DoubanBook> {
        let cache_key = id.to_string();
        if BOOK_CACHE.get(&cache_key).is_some() {
            return Ok(BOOK_CACHE.get(&cache_key).unwrap());
        }
        let url = format!("https://book.douban.com/subject/{}/", id);
        self.get_book_internal(url).await
    }

    fn get_text(&self, s: &str) -> String {
        s.split(':').collect::<Vec<&str>>()[1].trim().to_string()
    }

    fn get_texts(&self, array: &[&str], vec: &mut Vec<String>) {
        for e in array {
            if e.contains(':') {
                break;
            }
            if e == &"/" {
                continue;
            }
            vec.push(e.trim().to_string());
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubanBookResult<T> {
    code: u32,
    msg: String,
    books: Vec<T>,
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
    origin: String,           //原作名
}

pub struct SimpleDoubanBook {
    id: String,
    author: Vec<String>,
    images: Image,
    rating: Rating,
    pubdate: String,
    publisher: String,
    summary: String,
    title: String,
}

impl DoubanBook {
    fn simple(info: SimpleDoubanBook) -> DoubanBook {
        DoubanBook {
            id: info.id,
            author: info.author,
            author_intro: String::new(),
            translators: Vec::new(),
            images: info.images,
            binding: String::new(),
            category: String::new(),
            rating: info.rating,
            isbn13: String::new(),
            pages: String::new(),
            price: String::new(),
            pubdate: info.pubdate,
            publisher: info.publisher,
            producer: String::new(),
            serials: String::new(),
            subtitle: String::new(),
            summary: info.summary,
            title: info.title,
            tags: Vec::new(),
            origin: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    small: String,
    medium: String,
    large: String,
}

impl Image {
    fn new(large: String) -> Image {
        Image {
            large,
            medium: String::new(),
            small: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    average: f32,
}

impl Rating {
    fn new(rating: f32) -> Rating {
        Rating { average: rating }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HtmlResult {
    count: i32,
    html: String,
    limit: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookListItem {
    title: String,       //书名
    id: String,          //id
    author: Vec<String>, //作者
    pubdate: String,     //出版时间
    publisher: String,   //出版社
    images: Image,       //封面
    rating: Rating,      //评分
    summary: String,     //简介
}
