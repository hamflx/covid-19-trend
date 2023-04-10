use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PostInfo {
    title: String,
    date: String,
    link: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PostData {
    count: f32,
    count_date: String,
    positive_percent: String,
    positive_percent_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Data {
    data: Option<PostData>,
    post: PostInfo,
}

const MANIFEST_URL: &str = "https://www.chinacdc.cn/jkzt/crb/zl/szkb_11803/jszl_13141/";

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Safari/537.36";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

    println!("==> getting manifest ...");
    let res = client.get(MANIFEST_URL).send().await?;
    println!("==> parsing index ...");
    let body = res.text().await?;
    let document = Html::parse_document(&body);

    let first_item_selector =
        Selector::parse(".main .cn-main .cn-main-right .item-top .item-top-text").unwrap();
    let remain_items_selector =
        Selector::parse(".main .cn-main .cn-main-right .item-top .item-bottom ul li").unwrap();

    let first_item = document.select(&first_item_selector).next().unwrap();
    let remain_items = document.select(&remain_items_selector);
    let mut post_link_list = vec![first_item];
    post_link_list.append(&mut remain_items.collect::<Vec<_>>());

    let mut post_list = Vec::new();

    let base_url = Url::parse(MANIFEST_URL).unwrap();
    for item in post_link_list {
        let link = item
            .select(&Selector::parse("a").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("href")
            .unwrap()
            .to_string();
        let link = Url::options()
            .base_url(Some(&base_url))
            .parse(&link)
            .unwrap()
            .to_string();

        let title = item
            .select(&Selector::parse("a").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>()
            .trim()
            .to_string();
        let date = regex::Regex::new(r"\d{4}-\d{2}-\d{2}")
            .unwrap()
            .find(
                item.select(&Selector::parse("a + span").unwrap())
                    .next()
                    .unwrap()
                    .text()
                    .collect::<String>()
                    .trim(),
            )
            .unwrap()
            .as_str()
            .to_string();

        let post_info = PostInfo { title, date, link };
        post_list.push(post_info);
    }

    let mut stats = Vec::new();
    for post in post_list {
        println!("==> getting post {} ...", post.link);
        let post_data = get_post_data(&client, &post).await?;
        let data = Data {
            data: post_data,
            post,
        };
        stats.push(data);
    }

    let json = serde_json::to_string_pretty(&stats)?;
    std::fs::write("stats-rust.json", json).unwrap();

    Ok(())
}

async fn get_post_data(client: &Client, post: &PostInfo) -> anyhow::Result<Option<PostData>> {
    let res = client.get(&post.link).send().await?;
    let body = res.text().await?;
    let document = Html::parse_document(&body);

    let paragraphs_selector = Selector::parse(".TRS_Editor .TRS_Editor p").unwrap();
    let paragraphs = document.select(&paragraphs_selector);

    for p in paragraphs {
        let p_text = p
            .text()
            .collect::<String>()
            .replace("\n", "")
            .replace("\r", "")
            .replace(" ", "");
        let flag = "检测阳性率";
        let index_of_flag = p_text.find(flag);
        if let Some(index) = index_of_flag {
            let re_date_and_count =
                regex::Regex::new(r"((?:\d{4}年)?\d+月\d+日).{1,6}?((?:\d+\.)?\d+(?:万)?)")
                    .unwrap();
            let before_text = &p_text[..index];
            println!("==> before_text: {}", before_text);
            let (count_date, count) = before_text
                .split(|c| c == '，' || c == '；' || c == '。')
                .rev()
                .find_map(|seg| re_date_and_count.captures(seg))
                .map(|caps| (caps[1].to_string(), parse_number(&caps[2])))
                .unwrap();

            let after_text = &p_text[index + flag.len()..];
            let re_positive_percent =
                regex::Regex::new(r"((?:\d{4}年)?\d+月\d+日).{1,4}?((?:\d+\.)?(\d+)%?)").unwrap();
            let (positive_percent_date, positive_percent) = after_text
                .split(|c| c == '，' || c == '；' || c == '。')
                .rev()
                .find_map(|seg| re_positive_percent.captures(seg))
                .map(|caps| (caps[1].to_string(), caps[2].to_string()))
                .unwrap();

            let post_data = PostData {
                count,
                count_date,
                positive_percent,
                positive_percent_date,
            };
            return Ok(Some(post_data));
        }
    }

    Ok(None)
}

fn parse_number(num: &str) -> f32 {
    const SUFFIX: &str = "万";
    if num.ends_with(SUFFIX) {
        num[..num.len() - SUFFIX.len()].parse::<f32>().unwrap() * 10000f32
    } else {
        num.parse().unwrap()
    }
}
