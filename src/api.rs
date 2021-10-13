use anyhow::Result;
use lazy_static::*;
use moka::future::{Cache, CacheBuilder};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use urlencoding::encode;
use visdom::Vis;

lazy_static! {
    static ref MOVIE_CACHE: Cache<String, MovieInfo> = CacheBuilder::new(CACHE_SIZE)
        .time_to_live(Duration::from_secs(10 * 60))
        .build();
}

const ORIGIN: &str = "https://movie.douban.com";
const REFERER: &str = "https://movie.douban.com/";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";
const DEFAULT_LIMIT: usize = 3;
const CACHE_SIZE: usize = 100;

#[derive(Clone)]
pub struct Douban {
    client: reqwest::Client,
    search_limit_size: usize,
    re_id: Regex,
    re_backgroud_image: Regex,
    re_sid: Regex,
    re_cat: Regex,
    re_year: Regex,
    re_director: Regex,
    re_writer: Regex,
    re_actor: Regex,
    re_genre: Regex,
    re_country: Regex,
    re_language: Regex,
    re_duration: Regex,
    re_screen: Regex,
    re_subname: Regex,
    re_imdb: Regex,
    re_site: Regex,
}

impl Douban {
    pub fn new(limit_size: usize) -> Douban {
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
        let search_limit_size = if limit_size == 0 {
            DEFAULT_LIMIT
        } else {
            limit_size
        };

        let re_id = Regex::new(r"/(\d+?)/").unwrap();
        let re_backgroud_image = Regex::new(r"url\((.+?)\)").unwrap();
        let re_sid = Regex::new(r"sid: (\d+?),").unwrap();
        let re_cat = Regex::new(r"\[(.+?)\]").unwrap();
        let re_year = Regex::new(r"\((\d+?)\)").unwrap();
        let re_director = Regex::new(r"导演: (.+?)\n").unwrap();
        let re_writer = Regex::new(r"编剧: (.+?)\n").unwrap();
        let re_actor = Regex::new(r"主演: (.+?)\n").unwrap();
        let re_genre = Regex::new(r"类型: (.+?)\n").unwrap();
        let re_country = Regex::new(r"制片国家/地区: (.+?)\n").unwrap();
        let re_language = Regex::new(r"语言: (.+?)\n").unwrap();
        let re_duration = Regex::new(r"片长: (.+?)\n").unwrap();
        let re_screen = Regex::new(r"上映日期: (.+?)\n").unwrap();
        let re_subname = Regex::new(r"又名: (.+?)\n").unwrap();
        let re_imdb = Regex::new(r"IMDb: (.+?)\n").unwrap();
        let re_site = Regex::new(r"官方网站: (.+?)\n").unwrap();

        Self {
            client,
            search_limit_size,
            re_id,
            re_backgroud_image,
            re_sid,
            re_cat,
            re_year,
            re_director,
            re_writer,
            re_actor,
            re_genre,
            re_country,
            re_language,
            re_duration,
            re_screen,
            re_subname,
            re_imdb,
            re_site,
        }
    }

    pub async fn search(&self, q: &str, count: i32, proxy: &str) -> Result<Vec<Movie>> {
        let mut vec = Vec::with_capacity(self.search_limit_size);
        if q.is_empty() {
            return Ok(vec);
        }
        let mut num = self.search_limit_size;
        if count > 0 {
            num = count as usize;
        }

        let url = "https://www.douban.com/search";
        let res = self
            .client
            .get(url)
            .query(&[("q", q)])
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
                        let rating = x.find("div.rating-info>.rating_nums").text().to_string();
                        let onclick = x.find("div.title a").attr("onclick").unwrap().to_string();
                        let mut img = x.find("a.nbg>img").attr("src").unwrap().to_string();
                        if !proxy.is_empty() {
                            img = format!("{}?url={}", proxy, encode(&img));
                        }
                        let sid = self.parse_sid(&onclick);
                        let name = x.find("div.title a").text().to_string();
                        let title_mark = x.find("div.title>h3>span").text().to_string();
                        let cat = self.parse_cat(&title_mark);
                        let subject = x.find("div.rating-info>.subject-cast").text().to_string();
                        let year = self.parse_year(subject);
                        Movie {
                            cat,
                            sid,
                            name,
                            rating,
                            img,
                            year,
                        }
                    })
                    .into_iter()
                    .filter(|x| x.cat == "电影")
                    .take(num)
                    .collect::<Vec<Movie>>();
            }
            Err(err) => {
                println!("{:?}", err)
            }
        }

        Ok(vec)
    }

    pub async fn search_full(&self, q: &str, count: i32) -> Result<Vec<MovieInfo>> {
        let movies = self.search(q, count, "").await.unwrap();
        let mut list = Vec::with_capacity(movies.len());
        for i in movies.iter() {
            list.push(self.get_movie_info(&i.sid).await.unwrap())
        }

        Ok(list)
    }

    pub async fn get_movie_info(&self, sid: &str) -> Result<MovieInfo> {
        let cache_key = sid.to_string();
        if MOVIE_CACHE.get(&cache_key).is_some() {
            return Ok(MOVIE_CACHE.get(&cache_key).unwrap());
        }
        let url = format!("https://movie.douban.com/subject/{}/", sid);
        let res = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()
            .unwrap();

        let res = res.text().await?;
        let document = Vis::load(&res).unwrap();
        let x = document.find("#content");

        let sid = sid.to_string();
        let name = x.find("h1>span:first-child").text().to_string();
        let year_str = x.find("h1>span.year").text().to_string();
        let year = self.parse_year_for_detail(&year_str);

        let rating = x
            .find("div.rating_self strong.rating_num")
            .text()
            .to_string();
        let img = x.find("a.nbgnbg>img").attr("src").unwrap().to_string();

        let intro = x
            .find("div.indent>span")
            .text()
            .trim()
            .replace("©豆瓣", "")
            .to_string();
        let info = x.find("#info").text().to_string();
        let (
            director,
            writer,
            actor,
            genre,
            site,
            country,
            language,
            screen,
            duration,
            subname,
            imdb,
        ) = self.parse_info(&info);

        let celebrities: Vec<Celebrity> =
            x.find("#celebrities li.celebrity")
                .first()
                .map(|_index, x| {
                    let x = Vis::dom(x);
                    let id_str = x.find("div.info a.name").attr("href").unwrap().to_string();
                    let id = self.parse_id(&id_str);
                    let img_str = x.find("div.avatar").attr("style").unwrap().to_string();
                    let img = self.parse_backgroud_image(&img_str);
                    let name = x.find("div.info a.name").text().to_string();
                    let role = x.find("div.info span.role").text().to_string();

                    Celebrity {
                        id,
                        img,
                        name,
                        role,
                    }
                });

        let info = MovieInfo {
            sid,
            name,
            rating,
            img,
            year,
            intro,
            director,
            writer,
            actor,
            genre,
            site,
            country,
            language,
            screen,
            duration,
            subname,
            imdb,
            celebrities,
        };
        MOVIE_CACHE.insert(cache_key, info.clone()).await;

        Ok(info)
    }

    pub async fn get_celebrities(&self, sid: &str) -> Result<Vec<Celebrity>> {
        let url = format!("https://movie.douban.com/subject/{}/celebrities", sid);
        let res = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()
            .unwrap();

        let res = res.text().await?;
        let document = Vis::load(&res).unwrap();
        let x = document.find("#content");

        let celebrities: Vec<Celebrity> = x
            .find("ul.celebrities-list li.celebrity")
            .map(|_index, x| {
                let x = Vis::dom(x);
                let id_str = x.find("div.info a.name").attr("href").unwrap().to_string();
                let id = self.parse_id(&id_str);
                let img_str = x.find("div.avatar").attr("style").unwrap().to_string();
                let img = self.parse_backgroud_image(&img_str);
                let name = x
                    .find("div.info a.name")
                    .text()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let role = x
                    .find("div.info span.role")
                    .text()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();

                Celebrity {
                    id,
                    img,
                    name,
                    role,
                }
            })
            .into_iter()
            .filter(|x| x.role == "导演" || x.role == "配音" || x.role == "演员")
            .take(15)
            .collect::<Vec<Celebrity>>();

        Ok(celebrities)
    }

    pub async fn get_celebrity(&self, id: &str) -> Result<CelebrityInfo> {
        let url = format!("https://movie.douban.com/celebrity/{}/", id);
        let res = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()
            .unwrap();

        let res = res.text().await?;
        let document = Vis::load(&res).unwrap();
        let x = document.find("#content");
        let id = id.to_string();
        let img = x.find("a.nbg>img").attr("src").unwrap().to_string();
        let name = x.find("h1").text().to_string();
        let mut intro = x.find("#intro span.short").text().trim().to_string();
        if intro.is_empty() {
            intro = x.find("#intro div.bd").text().trim().to_string();
        }

        let info = x.find("div.info").text().to_string();
        let (gender, constellation, birthdate, birthplace, role, nickname, family, imdb) =
            self.parse_celebrity_info(&info);

        Ok(CelebrityInfo {
            id,
            img,
            name,
            role,
            intro,
            gender,
            constellation,
            birthdate,
            birthplace,
            nickname,
            imdb,
            family,
        })
    }

    pub async fn get_wallpaper(&self, sid: &str) -> Result<Vec<Photo>> {
        let url = format!("https://movie.douban.com/subject/{}/photos?type=W&start=0&sortby=size&size=a&subtype=a", sid);
        let res = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()
            .unwrap();

        let res = res.text().await?;
        let document = Vis::load(&res).unwrap();
        let wallpapers: Vec<Photo> = document.find(".poster-col3>li").map(|_index, x| {
            let x = Vis::dom(x);

            let id = x.attr("data-id").unwrap().to_string();
            let small = format!("https://img1.doubanio.com/view/photo/s/public/p{}.jpg", id);
            let medium = format!("https://img1.doubanio.com/view/photo/m/public/p{}.jpg", id);
            let large = format!("https://img1.doubanio.com/view/photo/l/public/p{}.jpg", id);
            let size = x.find("div.prop").text().trim().to_string();
            let mut width = String::new();
            let mut height = String::new();
            if !size.is_empty() {
                let arr: Vec<&str> = size.split('x').collect();
                width = arr[0].to_string();
                height = arr[1].to_string();
            }
            Photo {
                id,
                small,
                medium,
                large,
                size,
                width,
                height,
            }
        });

        Ok(wallpapers)
    }

    pub async fn proxy_img(&self, url: &str) -> Result<reqwest::Response> {
        Ok(self.client.get(url).send().await.unwrap())
    }

    fn parse_year(&self, text: String) -> String {
        text.split('/').last().unwrap().trim().to_string()
    }

    fn parse_year_for_detail(&self, text: &str) -> String {
        let mut year = String::new();
        for cap in self.re_year.captures_iter(text) {
            year = cap[1].to_string();
        }

        year
    }

    fn parse_sid(&self, text: &str) -> String {
        let mut sid = String::new();
        for cap in self.re_sid.captures_iter(text) {
            sid = cap[1].to_string();
        }

        sid
    }

    fn parse_cat(&self, text: &str) -> String {
        let mut sid = String::new();
        for cap in self.re_cat.captures_iter(text) {
            sid = cap[1].to_string();
        }

        sid
    }

    fn parse_id(&self, text: &str) -> String {
        let mut id = String::new();
        for cap in self.re_id.captures_iter(text) {
            id = cap[1].to_string();
        }

        id
    }

    fn parse_backgroud_image(&self, text: &str) -> String {
        let mut url = String::new();
        for cap in self.re_backgroud_image.captures_iter(text) {
            url = cap[1].to_string();
        }

        url
    }

    fn parse_info(
        &self,
        text: &str,
    ) -> (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) {
        let director = match self.re_director.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let writer = match self.re_writer.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let actor = match self.re_actor.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let genre = match self.re_genre.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let country = match self.re_country.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let language = match self.re_language.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let duration = match self.re_duration.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let screen = match self.re_screen.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let subname = match self.re_subname.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        let imdb = match self.re_imdb.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };
        let site = match self.re_site.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().to_string(),
            None => String::new(),
        };

        (
            director, writer, actor, genre, site, country, language, screen, duration, subname,
            imdb,
        )
    }

    fn parse_celebrity_info(
        &self,
        text: &str,
    ) -> (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) {
        let re_gender = Regex::new(r"性别: \n(.+?)\n").unwrap();
        let gender = match re_gender.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_constellation = Regex::new(r"星座: \n(.+?)\n").unwrap();
        let constellation = match re_constellation.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_birthdate = Regex::new(r"出生日期: \n(.+?)\n").unwrap();
        let birthdate = match re_birthdate.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_birthplace = Regex::new(r"出生地: \n(.+?)\n").unwrap();
        let birthplace = match re_birthplace.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_role = Regex::new(r"职业: \n(.+?)\n").unwrap();
        let role = match re_role.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_nickname = Regex::new(r"更多外文名: \n(.+?)\n").unwrap();
        let nickname = match re_nickname.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_family = Regex::new(r"家庭成员: \n(.+?)\n").unwrap();
        let family = match re_family.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        let re_imdb = Regex::new(r"imdb编号: \n(.+?)\n").unwrap();
        let imdb = match re_imdb.captures(text) {
            Some(x) => x.get(1).unwrap().as_str().trim().to_string(),
            None => String::new(),
        };

        (
            gender,
            constellation,
            birthdate,
            birthplace,
            role,
            nickname,
            family,
            imdb,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    cat: String,
    sid: String,
    name: String,
    rating: String,
    img: String,
    year: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieInfo {
    sid: String,
    name: String,
    rating: String,
    img: String,
    year: String,
    intro: String,
    director: String,
    writer: String,
    actor: String,
    genre: String,
    site: String,
    country: String,
    language: String,
    screen: String,
    duration: String,
    subname: String,
    imdb: String,
    pub celebrities: Vec<Celebrity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Celebrity {
    id: String,
    img: String,
    name: String,
    role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelebrityInfo {
    id: String,
    img: String,
    name: String,
    role: String,
    intro: String,
    gender: String,
    constellation: String,
    birthdate: String,
    birthplace: String,
    nickname: String,
    imdb: String,
    family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    id: String,
    small: String,
    medium: String,
    large: String,
    size: String,
    width: String,
    height: String,
}
