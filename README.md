# douban-api-rs
简单的豆瓣api，主要用于配合[jellyfin-plugin-opendouban](https://github.com/caryyu/jellyfin-plugin-opendouban)插件，在jellyfin中刮削电影信息


## 支持的api

```
/movies?q={movie_name}            # 搜索电影
/movies?q={movie_name}&type=full  # 搜索电影并获取详细信息
/movies/{sid}                     # 获取指定电影信息
/movies/{sid}/celebrities         # 获取演员列表
/celebrities/{cid}                # 获取演员信息
/photo/{sid}                      # 获取电影壁纸
```

## docker运行

```
docker run -d --name=douban-api-rs --restart=unless-stopped -p 8080:80 ghcr.io/cxfksword/douban-api-rs:latest
```


## 返回结果示例

搜索：

```
[
    {
        "cat": "电影",
        "sid": "26862259",
        "name": "乘风破浪 ",
        "rating": "6.8",
        "img": "https://img1.doubanio.com/view/photo/s_ratio_poster/public/p2408407697.jpg",
        "year": " 2017"
    },
    {
        "cat": "电影",
        "sid": "34894589",
        "name": "乘风破浪的姐姐 第一季 ",
        "rating": "6.8",
        "img": "https://img1.doubanio.com/view/photo/s_ratio_poster/public/p2608297477.jpg",
        "year": "2020"
    }
]
```


获取电影信息：

```
{
    "sid": "26862259",
    "name": "乘风破浪",
    "rating": "6.8",
    "img": "https://img1.doubanio.com/view/photo/s_ratio_poster/public/p2408407697.jpg",
    "year": "2017",
    "intro": "赛车手阿浪（邓超 饰）一直对父亲（彭于晏 饰）反对自己的赛车事业耿耿于怀，在向父亲证明自己的过程中，阿浪却意外卷入了一场奇妙的冒险。他在这段经历中结识了一群兄弟好友，一同闯过许多奇幻的经历，也对自己的身世有了更多的了解。",
    "director": "导演",
    "writer": "编剧",
    "actor": "主演",
    "genre": "类型",
    "site": "",
    "country": "制片国家/地区",
    "language": "语言",
    "screen": "上映日期",
    "duration": "片长",
    "subname": "上映日期",
    "imdb": "IMDb",
    "celebrities": [
        {
            "id": "1275307",
            "img": "https://img3.doubanio.com/view/celebrity/raw/public/p42220.jpg",
            "name": "韩寒",
            "role": "导演"
        }
    ]
}
```

获取演员信息：

```
{
    "id": "1274235",
    "img": "https://img2.doubanio.com/icon/u183170142-13.jpg",
    "name": "邓超 Chao Deng",
    "role": "演员 / 导演 / 配音 / 主持人",
    "intro": "1979年，邓超出生在一个重新组合的小康家庭，爸爸是博物...",
    "gender": "男",
    "constellation": "水瓶座",
    "birthdate": "1979年02月08日",
    "birthplace": "中国,江西,南昌",
    "nickname": "",
    "imdb": "nm2874732",
    "family": "孙俪(妻)"
}
```