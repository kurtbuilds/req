use std::borrow::Cow;
use async_trait::async_trait;
use clap::{Arg, Values};
use colored::Colorize;
use httpclient::middleware::{FollowRedirectsMiddleware, Next};
use httpclient::{Body, Error, Middleware, Request, Response};
use std::fs;
use std::str::FromStr;
use colored_json::ToColoredJson;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

pub struct VerboseMiddleware;

#[async_trait]
impl Middleware for VerboseMiddleware {
    async fn handle(&self, request: Request, next: Next<'_>) -> Result<Response, Error> {
        eprintln!("{} {}", request.method(), request.url());
        if !request.headers().is_empty() {
            eprintln!("Headers:");
        }
        for (key, value) in request.headers() {
            eprintln!("{}: {}", key, value.to_str().unwrap());
        }
        if !request.body().is_empty() {
            eprintln!("Body:");
            match request.body().try_clone().unwrap() {
                Body::Empty => {}
                Body::Bytes(b) => println!("<{} bytes>", b.len()),
                Body::Text(s) => println!("{}", s),
                Body::Hyper(_) => {}
                Body::Json(j) => println!("{}", serde_json::to_string_pretty(&j).unwrap()),
            };
        }
        eprintln!("==========");
        let res = next.run(request).await;
        match &res {
            Ok(res) => {
                eprintln!("{}", res.status());
                if !res.headers().is_empty() {
                    eprintln!("Headers:");
                }
                for (key, value) in res.headers() {
                    eprintln!("{}: {}", key, value.to_str().unwrap());
                }
            }
            Err(err) => eprintln!("{:?}", err),
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

pub fn split_pair<'a>(pair: &'a str, sep: &[char]) -> Option<(&'a str, &'a str)> {
    let mut iter = pair.splitn(2, sep);
    if let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        Some((a, b))
    } else {
        None
    }
}


fn build_map(values: Values) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for pair in values {
        let (key, value) = pair.split_once(&['=', ':']).unwrap();
        let mut parts = key.split('.');
        let mut current = &mut map;
        while let Some(part) = parts.next() {
            if parts.next().is_none() {
                current.insert(part.to_string(), serde_json::Value::String(value.to_string()));
            } else {
                current = current.entry(part.to_string()).or_insert_with(|| serde_json::Value::Object(serde_json::Map::new())).as_object_mut().unwrap();
            }
        }
    }
    serde_json::Value::Object(map)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
                    "req localhost:5000/signup --json email=test@example.com password=test",
                ),
                (
                    "Sends a JSON POST request with URL params. URL params before --json, JSON body after --json.",
                    "req localhost:5000/search cache=0 --json query='search query'",
                ),
            ])
                .as_str(),
        )
        .arg(Arg::new("headers")
            .multiple_occurrences(true)
            .takes_value(true)
            .long("header")
            .short('H')
            .help("Sets a header. Can be used multiple times. Separator can be `:` or `=`. Example: `-H content-type:application/json` or `-H 'accept=*/*'`")
        )
        .arg(Arg::new("raw")
            .takes_value(true)
            .long("raw")
            .short('r')
            .help("By default, req makes an effort to pretty print the results. Right now, we only pretty print JSON. ")
        )
        .arg(Arg::new("bearer")
            .takes_value(true)
            .long("bearer")
            .help("Sets header `Authorization: Bearer <value>`.")
        )
        .arg(Arg::new("token")
            .takes_value(true)
            .long("token")
            .help("Sets header `Authorization: Token <value>`.")
        )
        .arg(Arg::new("remote-name")
            .short('O')
            .long("remote-name")
            .help("Behaves like curl -O. Save the response to a file with the same name as the remote URL.")
        )
        .arg(Arg::new("verbose").long("verbose").short('v'))
        .arg(Arg::new("method")
            .long("method")
            .short('m')
            .takes_value(true)
            .help("Sets the request method. Defaults to GET. Behaves like curl -X.")
        )
        .arg(Arg::new("user")
            .long("user")
            .short('u')
            .takes_value(true)
            .help("Sets user authentication header. Behaves like curl -u. Example: `-u username:password`. `username:password` is base64-encoded, and header `Authorization: Basic <base64>` is set.")
        )
        .arg(Arg::new("url")
            .required(true)
            .help("<url> is permissive for valid values. Can be :5000, localhost:3000, https://www.google.com, etc.")
        )
        .arg(Arg::new("params")
            .multiple_occurrences(true)
            .help("Sets URL query params. It urlencodes the provided values.")
        )
        .arg(
            Arg::new("json")
                .takes_value(true)
                .multiple_values(true)
                .long("json")
                .help("Sets JSON body. --json is greedy, so every value after it is treated as a json key/value pair."),
        )
        .arg(
            Arg::new("form")
                .takes_value(true)
                .multiple_values(true)
                .long("form")
                .help("Sets form body. --form is greedy, so every value after it is treated as a form key/value pair."),
        )
        .get_matches();

    let mut url = matches.value_of("url").unwrap().to_string();
    if !url.starts_with("http") {
        if url.starts_with(":") {
            url = format!("localhost{}", url);
        }
        url = format!("http://{}", url);
    }

    let params = matches
        .values_of("params")
        .unwrap_or_default()
        .map(|v| split_pair(v, &['=', ':']).expect("Params must be in the form of key=value or key:value"))
        .collect::<Vec<_>>();

    let mut headers = matches
        .values_of("headers")
        .unwrap_or_default()
        .map(|v| split_pair(v, &['=', ':']).expect("Headers must be in the form of key=value or key:value"))
        .map(|(k, v)| (k, Cow::Borrowed(v)))
        .collect::<Vec<_>>();

    // Set bearer
    if let Some(bearer) = matches.value_of("bearer") {
        headers.push(("Authorization", Cow::Owned(format!("Bearer {}", bearer))));
    }

    // Set token
    if let Some(token) = matches.value_of("token") {
        headers.push(("Authorization", Cow::Owned(format!("Token {}", token))));
    }

    // Set user
    if let Some(user) = matches.value_of("user") {
        let base64 = base64::encode(user);
        headers.push(("Authorization", Cow::Owned(format!("Basic {}", base64))));
    }

    let method = matches
        .value_of("method")
        .map(|v| httpclient::Method::from_str(&v.to_uppercase()).expect("Method must be one of: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, TRACE, CONNECT"))
        .unwrap_or_else(|| {
            if matches.is_present("json") || matches.is_present("form") {
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
        let obj = build_map(json);
        builder = builder.push_json(obj);
        if !headers.iter().any(|(h, _)| h.to_lowercase() == "accept") {
            headers.push(("Accept", Cow::Borrowed("application/json")));
        }
    };

    if let Some(form) = matches.values_of("form") {
        let obj = build_map(form);
        builder = builder.set_body(Body::Text(serde_urlencoded::to_string(&obj).expect("Failed to encode as form-urlencoded.")));
        headers.push(("Content-Type", Cow::Borrowed("application/x-www-form-urlencoded")));
        headers.push(("Accept", Cow::Borrowed("*/*")));
    };
    let raw = matches.is_present("raw");

    builder = builder.headers(headers.clone().iter().map(|(k, v)| (*k, v.as_ref())));
    let res = builder.send().await.unwrap();
    if matches.is_present("remote-name") {
        let url = httpclient::Uri::from_str(&url).unwrap();
        let filename = std::path::Path::new(url.path())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let bytes = res.bytes().await.unwrap();
        fs::write(filename, bytes).expect("Failed to write to file.");
    } else {
        let expect_json = res.headers().get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.starts_with("application/json"))
            .unwrap_or(false);
        let mut s = res.text().await.unwrap();
        if !raw && expect_json {
            s = s.to_colored_json_auto()?;
        }
        println!("{}", s);
    }
    Ok(())
}

#[cfg(test)]
mod tests {}
