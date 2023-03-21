mod middleware;

use std::borrow::Cow;
use clap::{Parser};
use colored::Colorize;
use httpclient::middleware::{FollowRedirectsMiddleware};
use httpclient::{InMemoryBody};
use std::{fs};
use std::str::FromStr;
use base64::Engine;
use colored_json::ToColoredJson;
use middleware::VerboseMiddleware;

static EXAMPLES: &[(&'static str, &'static str)] = &[
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
];


pub fn examples(pairs: &[(&'static str, &'static str)]) -> String {
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

#[derive(Parser, Debug)]
#[command(after_help(examples(EXAMPLES)))]
#[command(author, version, about)]
struct Cli {
    #[arg(help = "<url> is permissive for valid values. Can be :5000, localhost:3000, https://www.google.com, etc.")]
    url: String,

    #[arg(help = "Sets URL query params. It urlencodes the provided values.")]
    params: Vec<String>,

    #[arg(long, num_args = 1.., help = r#"Sets JSON body. --json is greedy, so every value after it is treated as a json key/value pair. Dots in keys are treated as nested objects. For example, --json foo.bar=1 foo.baz=2 will result in {"foo": {"bar": 1, "baz": 2}}"#)]
    json: Option<Vec<String>>,

    #[arg(long, num_args = 1.., help = "Sets Form body. --form is greedy, so every value after it is treated as a form key/value pair.")]
    form: Option<Vec<String>>,

    #[arg(short, long, help = "Sets the request method. Defaults to GET. Behaves like curl -X.")]
    method: Option<String>,

    #[arg(short, long)]
    verbose: bool,

    #[arg(short, long, help = "By default, req makes an effort to pretty print the results. Right now, we only pretty print JSON.")]
    raw: bool,

    #[arg(long, help = "By default, req will error if it receives a >= 400 status code. This flag turns off that behavior.")]
    ignore_status: bool,

    #[arg(long, help = "Sets header `Authorization: Bearer <value>`.")]
    bearer: Option<String>,

    #[arg(long, help = "Sets header `Authorization: Token <value>`.")]
    token: Option<String>,

    #[arg(short, long, help = "Sets user authentication header. Behaves like curl -u. Example: `-u user:pass` provides header `Authorization: Basic $(base64 user:pass)`.")]
    user: Option<String>,

    #[arg(short = 'H', long, help = "Sets a header. Can be used multiple times. Separator can be `:` or `=`. Example: `-H content-type:application/json` or `-H 'accept=*/*'`")]
    headers: Vec<String>,

    #[arg(short = 'c', long = "cookie", help = "Set a cookie.")]
    cookies: Vec<String>,

    #[arg(short = 'O', long, help = "Behaves like curl -O. Save the response to a file with the same name as the remote URL.")]
    remote_name: bool,

    #[arg(short = 'F', long, help = "By default, req follows redirects. This flag disables that behavior.")]
    no_follow: bool,

    #[arg(long)]
    file: Option<String>,
}


pub fn split_pair<'a>(pair: &'a str, sep: &[char]) -> Option<(&'a str, &'a str)> {
    let mut iter = pair.splitn(2, sep);
    if let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        Some((a, b))
    } else {
        None
    }
}


fn build_map<'a>(values: impl Iterator<Item=&'a str>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for pair in values {
        let (key, value) = pair.split_once(&['=', ':']).unwrap();
        let mut parts = key.split('.').peekable();
        let mut current = &mut map;
        // 1. part=credential, parts=username
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                let value = serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()));
                current.insert(part.to_string(), value);
            } else {
                current = current.entry(part.to_string()).or_insert_with(|| serde_json::Value::Object(serde_json::Map::new())).as_object_mut().unwrap();
            }
        }
    }
    serde_json::Value::Object(map)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    sigpipe::reset();
    let cli = Cli::parse();

    let mut url = cli.url;
    if !url.starts_with("http") {
        if url.starts_with(":") {
            url = format!("localhost{}", url);
        }
        url = format!("http://{}", url);
    }

    let params = cli.params
            .iter()
            .map(|v| split_pair(v.as_str(), &['=', ':']).expect("Params must be in the form of key=value or key:value"))
            .collect::<Vec<_>>();

    let mut headers = cli.headers
        .iter()
        .map(|v| split_pair(v, &['=', ':']).expect("Headers must be in the form of key=value or key:value"))
        .map(|(k, v)| (k, Cow::Borrowed(v)))
        .collect::<Vec<_>>();

    // Set bearer
    if let Some(bearer) = cli.bearer {
        headers.push(("Authorization", Cow::Owned(format!("Bearer {}", bearer))));
    }

    // Set token
    if let Some(token) = cli.token {
        headers.push(("Authorization", Cow::Owned(format!("Token {}", token))));
    }

    // Set user
    if let Some(user) = cli.user {
        let base64 = base64::engine::general_purpose::STANDARD.encode(&user);
        headers.push(("Authorization", Cow::Owned(format!("Basic {}", base64))));
    }

    if !cli.cookies.is_empty() {
        headers.push(("Cookie", Cow::Owned(cli.cookies.join("; "))));
    }

    // Set method
    let method = cli.method
        .map(|v| httpclient::Method::from_str(&v.to_uppercase()).expect("Method must be one of: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, TRACE, CONNECT"))
        .unwrap_or_else(|| {
            if cli.json.is_some() || cli.form.is_some() {
                httpclient::Method::POST
            } else {
                httpclient::Method::GET
            }
        });

    let mut client = httpclient::Client::new();

    if !cli.no_follow {
        client = client.with_middleware(FollowRedirectsMiddleware {});
    }

    if cli.verbose {
        client = client.with_middleware(VerboseMiddleware {});
    }

    let mut builder = client.request(method.clone(), &url);

    // Set params
    for (k, v) in params {
        builder = builder.query(&k, &v);
    }

    // Set Json
    if let Some(json) = cli.json {
        let obj = build_map(json.iter().map(|s| s.as_str()));
        builder = builder.set_json(obj);
        if !headers.iter().any(|(h, _)| h.to_lowercase() == "accept") {
            headers.push(("Accept", Cow::Borrowed("application/json")));
        }
    };

    // Set form
    if let Some(form) = cli.form {
        let obj = build_map(form.iter().map(|s| s.as_str()));
        builder = builder.body(InMemoryBody::Text(serde_urlencoded::to_string(&obj).expect("Failed to encode as form-urlencoded.")));
        headers.push(("Content-Type", Cow::Borrowed("application/x-www-form-urlencoded")));
        headers.push(("Accept", Cow::Borrowed("*/*")));
    };

    if let Some(fpath) = cli.file {
        let file = fs::read(&fpath).expect("Failed to read file.");
        builder = builder.header("Content-Length", &file.len().to_string());
        builder = builder.header("Content-Type", mime_guess::from_path(&fpath).first_or_octet_stream().as_ref());
        builder = builder.body(InMemoryBody::Bytes(file));
    }

    // Add headers
    builder = builder.headers(headers.clone().iter().map(|(k, v)| (*k, v.as_ref())));

    // Make the request
    let res = builder.send().await.unwrap();

    if !cli.ignore_status && !res.status().is_success() {
        let expect_json = res.headers().get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.starts_with("application/json"))
            .unwrap_or(false);
        let mut s = res.text().await.unwrap();
        if !cli.raw && expect_json {
            s = s.to_colored_json_auto().unwrap();
        }
        println!("{}", s);
        std::process::exit(1);
    }

    if cli.remote_name {
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
        if !cli.raw && expect_json {
            s = s.to_colored_json_auto().unwrap();
        }
        println!("{}", s);
    }
}

#[cfg(test)]
mod tests {
    use crate::build_map;

    #[test]
    fn test_build_map() {
        let v = vec![
            "credential.username=test@gmail.com",
            "credential.password=foo",
        ];
        let result = build_map(v.into_iter());
        assert_eq!(result["credential"]["username"], "test@gmail.com");
        assert_eq!(result["credential"]["password"], "foo");
    }

    #[test]
    fn test_build_map_bool() {
        let v = vec![
            "a=true",
            "b=abc123",
            "c={",
            "d=5",
            "e=-5.5",
        ];
        let result = build_map(v.into_iter());
        assert_eq!(result["a"], true);
        assert_eq!(result["b"], "abc123");
        assert_eq!(result["c"], "{");
        assert_eq!(result["d"], 5);
        assert_eq!(result["e"], -5.5);
    }
}
