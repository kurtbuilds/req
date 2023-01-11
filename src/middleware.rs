use async_trait::async_trait;
use httpclient::{Body, Error, Middleware, Request, Response};
use httpclient::middleware::Next;

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
