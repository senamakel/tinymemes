use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[test]
fn compatible_client_debug_omits_credentials_but_keeps_route_identity() {
    let model = OpenAiCompatible::new(
        reqwest::Client::new(),
        "http://127.0.0.1:1/api",
        "sensitive-test-key",
        "fixture-model",
    );
    let debug = format!("{model:?}");
    assert!(debug.contains("http://127.0.0.1:1/api"));
    assert!(debug.contains("fixture-model"));
    assert!(!debug.contains("sensitive-test-key"));
}

async fn serve_once(body: &'static str, status: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        let n = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..n]);
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-key")
        );
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{addr}/v1")
}

#[tokio::test]
async fn compatible_chat_sends_exchange_and_returns_first_message() {
    let base = serve_once(r#"{"choices":[{"message":{"content":"  rewritten  "}},{"message":{"content":"ignored"}}]}"#, "200 OK").await;
    let model = OpenAiCompatible::new(reqwest::Client::new(), base, "test-key", "fixture-model");
    assert_eq!(
        model.complete("system rule", "user text").await.unwrap(),
        "  rewritten  "
    );
}

#[tokio::test]
async fn compatible_chat_rejects_http_and_empty_content_responses() {
    let base = serve_once("{}", "503 Service Unavailable").await;
    let model = OpenAiCompatible::new(reqwest::Client::new(), base, "test-key", "fixture-model");
    assert!(model.complete("s", "u").await.is_err());

    let base = serve_once(r#"{"choices":[{"message":{"content":"  "}}]}"#, "200 OK").await;
    let model = OpenAiCompatible::new(reqwest::Client::new(), base, "test-key", "fixture-model");
    assert_eq!(
        model.complete("s", "u").await.unwrap_err().to_string(),
        "model returned no content"
    );
}
