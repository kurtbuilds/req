use async_trait::async_trait;
use clap::Arg;
use colored::Colorize;
use httpclient::middleware::{FollowRedirectsMiddleware, Next};
use httpclient::{Body, Error, Middleware, Request, Response};
use std::fs;
use std::str::FromStr;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

pub struct VerboseMiddleware;

#[async_trait]
impl Middleware for VerboseMiddleware {
    async fn handle(&self, request: Request, next: Next<'_>) -> Result<Response, Error> {
        println!("{} {}", request.method(), request.url());
        if !request.headers().is_empty() {
            println!("Headers:");
        }
        for (key, value) in request.headers() {
            println!("{}: {}", key, value.to_str().unwrap());
        }
        if !request.body().is_empty() {
            println!("Body:");
            match request.body().try_clone().unwrap() {
                Body::Empty => {}
                Body::Bytes(b) => println!("<{} bytes>", b.len()),
                Body::Text(s) => println!("{}", s),
                Body::Hyper(_) => {}
                Body::Json(j) => println!("{}", serde_json::to_string_pretty(&j).unwrap()),
            };
        }
        println!("==========");
        let res = next.run(request).await;
        match &res {
            Ok(res) => {
                println!("{}", res.status());
                if !res.headers().is_empty() {
                    println!("Headers:");
                }
                for (key, value) in res.headers() {
                    println!("{}: {}", key, value.to_str().unwrap());
                }
            }
            Err(err) => println!("{:?}", err),
        }
        res
    }
}

pub fn examples(pairs: Vec<(&'static str, &'static str)>) -> String {
    format!(
        "{}
    {}",
        "EXAMPLES:".yellow(),
        pairs
            .iter()
            .map(|(a, b)| format!("{}\n    {}", format!("# {}", a).dimmed(), b))
            .collect::<Vec<String>>()
            .join("\n\n    "),
    )
}

pub fn split_pair(pair: &str) -> (&str, &str) {
    let mut iter = pair.splitn(2, '=');
    (iter.next().unwrap(), iter.next().unwrap())
}

#[tokio::main]
async fn main() {
    let matches = clap::Command::new(NAME)
        .version(VERSION)
        .arg_required_else_help(true)
        .after_help(
            examples(vec![
                ("Plain GET request", "req jsonip.com"),
                (
                    "GET request with a URL encoded string",
                    "req jsonip.com apiKey='foo bar'",
                ),
                (
                    "Sends a POST request with a JSON body.",
                    "req --post localhost:5000/signup email=test@example.com password=test",
                ),
            ])
            .as_str(),
        )
        .arg(
            Arg::new("headers")
                .multiple_occurrences(true)
                .takes_value(true)
                .long("header")
                .short('H'),
        )
        .arg(Arg::new("remote-name").short('O').long("remote-name"))
        .arg(Arg::new("verbose").long("verbose").short('v'))
        .arg(
            Arg::new("method")
                .long("method")
                .short('m')
                .takes_value(true),
        )
        .arg(Arg::new("url").required(true))
        .arg(Arg::new("params").multiple_occurrences(true))
        .arg(
            Arg::new("json")
                .takes_value(true)
                .multiple_values(true)
                .long("json")
                .short('j'),
        )
        .get_matches();

    let mut url = matches.value_of("url").unwrap().to_string();
    if !url.starts_with("http") {
        url = format!("http://{}", url);
    }
    let params = matches
        .values_of("params")
        .unwrap_or_default()
        .map(split_pair)
        .collect::<Vec<_>>();
    let headers = matches
        .values_of("headers")
        .unwrap_or_default()
        .map(split_pair)
        .collect::<Vec<_>>();
    let method = matches
        .value_of("method")
        .map(|v| httpclient::Method::from_str(v).unwrap())
        .unwrap_or_else(|| {
            if matches.is_present("json") {
                httpclient::Method::POST
            } else {
                httpclient::Method::GET
            }
        });
    let mut client = httpclient::Client::new(None).with_middleware(FollowRedirectsMiddleware {});
    if matches.is_present("verbose") {
        client = client.with_middleware(VerboseMiddleware {});
    }
    let mut builder = client.request(method.clone(), &url);
    for (k, v) in params {
        builder = builder.push_query(&k, &v);
    }
    if let Some(json) = matches.values_of("json") {
        let obj = json
            .map(split_pair)
            .fold(serde_json::Map::new(), |mut acc, (k, v)| {
                acc.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                acc
            });
        builder = builder.push_json(serde_json::Value::Object(obj));
    };
    builder = builder.headers(headers.clone().into_iter());
    let res = builder.send().await.unwrap();
    if matches.is_present("remote-name") {
        let url = httpclient::Uri::from_str(&url).unwrap();
        let filename = std::path::Path::new(url.path())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let bytes = res.bytes().await.unwrap();
        fs::write(filename, bytes).unwrap();
    } else {
        println!("{}", res.text().await.unwrap());
    }
}
