use async_trait::async_trait;
use httpclient::{InMemoryBody, InMemoryRequest, Middleware, ProtocolResult, Response};
use httpclient::middleware::Next;

#[derive(Debug)]
pub struct VerboseMiddleware;

#[async_trait]
impl Middleware for VerboseMiddleware {
    async fn handle(&self, request: InMemoryRequest, next: Next<'_>) -> ProtocolResult<Response> {
        eprintln!("{} {}", request.method(), request.uri());
        if !request.headers().is_empty() {
            eprintln!("Headers:");
        }
        for (key, value) in request.headers() {
            eprintln!("{}: {}", key, value.to_str().unwrap());
        }
        if !request.body().is_empty() {
            eprintln!("Body:");
            match request.body() {
                InMemoryBody::Bytes(b) => eprintln!("<{} bytes>", b.len()),
                InMemoryBody::Empty => {}
                InMemoryBody::Text(s) => eprintln!("{}", s),
                InMemoryBody::Json(j) => eprintln!("{}", serde_json::to_string_pretty(&j).expect("Failed to serialize JSON")),
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
