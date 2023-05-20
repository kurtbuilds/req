use async_trait::async_trait;
use httpclient::{Body, Error, InMemoryBody, Middleware, Request, Response};
use httpclient::middleware::Next;

#[derive(Debug)]
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
            match request.body() {
                Body::InMemory(InMemoryBody::Bytes(b)) => eprintln!("<{} bytes>", b.len()),
                Body::InMemory(InMemoryBody::Empty) => {}
                Body::InMemory(InMemoryBody::Text(s)) => eprintln!("{}", s),
                Body::InMemory(InMemoryBody::Json(j)) => eprintln!("{}", serde_json::to_string_pretty(&j).unwrap()),
                Body::Hyper(_) => {}
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
