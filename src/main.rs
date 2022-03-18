use clap::Arg;
use httpclient::{Body, Error, Middleware, Request, Response};
use httpclient::middleware::{FollowRedirectsMiddleware, LoggerMiddleware, Next};
use serde_json::json;
use async_trait::async_trait;
use colored::Colorize;


const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

pub struct VerboseMiddleware;

#[async_trait]
impl Middleware for VerboseMiddleware {
    async fn handle(&self, request: Request, mut next: Next<'_>) -> Result<Response, Error> {
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
                Body::Bytes(b) => {}
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
    format!("{}
    {}",
            "EXAMPLES:".yellow(),
            pairs.iter()
                .map(|(a, b)| format!("{}\n    {}", format!("# {}", a).dimmed(), b))
                .collect::<Vec<String>>().join("\n\n    "),
    )
}

#[tokio::main]
async fn main() {
    let matches = clap::Command::new(NAME)
        .version(VERSION)
        .arg_required_else_help(true)
        .after_help(examples(vec![
            ("Plain GET request", "req jsonip.com"),
            ("GET request with a URL encoded string", "req jsonip.com apiKey='foo bar'"),
            ("Sends a POST request with a JSON body.", "req --post localhost:5000/signup email=test@example.com password=test"),
        ]).as_str())
        .arg(Arg::new("headers")
            .multiple_occurrences(true)
            .takes_value(true)
            .long("header")
            .short('H')
        )
        .arg(Arg::new("verbose")
            .long("verbose")
            .short('v')
        )
        .arg(Arg::new("post")
            .long("post")
        )
        .arg(Arg::new("url")
            .required(true))
        .arg(Arg::new("params")
            .multiple_occurrences(true)
        )

        .get_matches();

    let mut url = matches.value_of("url").unwrap().to_string();
    if !url.starts_with("http") {
        url = format!("http://{}", url);
    }
    let params = matches.values_of("params").unwrap_or_default()
        .map(|v| {
            let mut split = v.splitn(2, '=');
            let k = split.next().unwrap();
            let v = split.next().unwrap();
            (k, v)
        })
        .collect::<Vec<_>>();
    let headers = matches.values_of("headers").unwrap_or_default()
        .map(|v| {
            let mut split = v.splitn(2, '=');
            let k = split.next().unwrap();
            let v = split.next().unwrap();
            (k, v)
        })
        .collect::<Vec<_>>();
    let method = if matches.is_present("post") {
        httpclient::Method::POST
    } else {
        httpclient::Method::GET
    };
    let client = httpclient::Client::new(None)
        .with_middleware(FollowRedirectsMiddleware {})
        .with_middleware(VerboseMiddleware {})
        ;

    let mut builder = client.request(method.clone(), &url);
    match method.as_str() {
        "GET" => {
            for (k, v) in params {
                builder = builder.push_query(k, v);
            }
        }
        "POST" => {
            for (k, v) in params {
                builder = builder.push_json(json!({k: v}));
            }
        }
        _ => panic!("Unsupported method"),
    };
    builder = builder.headers(headers.clone().into_iter());
    let res = builder
        .send()
        .await
        .unwrap();

    println!("{}", res.text().await.unwrap());
}
