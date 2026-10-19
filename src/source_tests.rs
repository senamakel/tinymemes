use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn template(name: &str) -> Meme {
    Meme {
        title: name.to_owned(),
        url: format!("https://i.imgflip.com/{}.jpg", name.len()),
        source: "imgflip".into(),
        meaning: None,
    }
}

#[tokio::test]
async fn imgflip_prefers_intent_hints_then_query_words() {
    let src = Imgflip::with_templates(vec![
        template("Drake Hotline Bling"),
        template("This Is Fine"),
        template("Fine Dining Cat"),
        template("Two Buttons"),
    ]);
    let hits = src
        .search(
            "everything is fine",
            Intent::Frustration,
            &Region::global(),
            5,
        )
        .await
        .unwrap();
    let names: Vec<_> = hits.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(names, ["This Is Fine", "Fine Dining Cat"]);
}

#[tokio::test]
async fn imgflip_fetches_templates_once_then_reuses_the_cached_list() {
    let (endpoint, request) = mock_endpoint(
        r#"{"success":true,"data":{"memes":[{"name":"Drake","url":"https://img.test/drake.jpg"},{"name":"Fine Dining Cat","url":"https://img.test/cat.jpg"}]}}"#,
        "200 OK",
    )
    .await;
    let mut source = Imgflip::new(reqwest::Client::new());
    source.endpoint = endpoint;
    let first = source
        .search("drake", Intent::Banter, &Region::global(), 5)
        .await
        .unwrap();
    let second = source
        .search("dining", Intent::Banter, &Region::global(), 5)
        .await
        .unwrap();
    assert_eq!(first[0].title, "Drake");
    assert_eq!(second[0].title, "Fine Dining Cat");
    assert!(request.await.unwrap().starts_with("GET /search HTTP/1.1"));
}

async fn mock_endpoint(
    body: &'static str,
    status: &'static str,
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        let n = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..n]).to_string();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        request
    });
    (format!("http://{addr}/search"), task)
}

#[tokio::test]
async fn provider_sources_parse_successful_responses_and_apply_region_filters() {
    let (endpoint, request) = mock_endpoint(r#"{"success":true,"data":{"memes":[{"name":"Drake","url":"https://img.test/drake.jpg"}]}}"#, "200 OK").await;
    let mut imgflip = Imgflip::new(reqwest::Client::new());
    imgflip.endpoint = endpoint;
    assert_eq!(
        imgflip
            .search("drake", Intent::Banter, &Region::global(), 5)
            .await
            .unwrap()[0]
            .title,
        "Drake"
    );
    assert!(request.await.unwrap().starts_with("GET /search HTTP/1.1"));

    let (endpoint, request) = mock_endpoint(r#"{"data":[{"title":"reaction","images":{"downsized":{"url":"https://media.test/a.gif"}}}]}"#, "200 OK").await;
    let mut giphy = Giphy::new(reqwest::Client::new(), "g-key");
    giphy.endpoint = endpoint;
    let gif = giphy
        .search("hello world", Intent::Banter, &Region::india(), 3)
        .await
        .unwrap();
    assert_eq!(gif[0].source, "giphy");
    let query = request.await.unwrap();
    assert!(query.contains("rating=pg-13"));
    assert!(query.contains("lang=hi"));

    let (endpoint, request) = mock_endpoint(r#"{"results":[{"content_description":"laugh","media_formats":{"tinygif":{"url":"https://media.test/a.gif"}}}]}"#, "200 OK").await;
    let mut tenor = Tenor::new(reqwest::Client::new(), "t-key");
    tenor.endpoint = endpoint;
    let result = tenor
        .search("laugh", Intent::Banter, &Region::india(), 2)
        .await
        .unwrap();
    assert_eq!(result[0].title, "laugh");
    let query = request.await.unwrap();
    assert!(query.contains("contentfilter=medium"));
    assert!(query.contains("media_filter=tinygif"));
    assert!(query.contains("country=IN"));
}

#[tokio::test]
async fn provider_failures_do_not_cache_or_accept_bad_payloads() {
    let (endpoint, _) = mock_endpoint(r#"{"success":false,"data":{"memes":[]}}"#, "200 OK").await;
    let mut imgflip = Imgflip::new(reqwest::Client::new());
    imgflip.endpoint = endpoint;
    assert!(
        imgflip
            .search("anything", Intent::Banter, &Region::global(), 3)
            .await
            .is_err()
    );

    let (endpoint, _) = mock_endpoint("{}", "503 Service Unavailable").await;
    let mut tenor = Tenor::new(reqwest::Client::new(), "key");
    tenor.endpoint = endpoint;
    assert!(
        tenor
            .search("q", Intent::Banter, &Region::global(), 1)
            .await
            .is_err()
    );
}

#[test]
fn meme_serialization_omits_unknown_meaning_but_preserves_known_context() {
    let meme = template("Drake");
    let value = serde_json::to_value(&meme).unwrap();
    assert!(value.get("meaning").is_none());
    let with_meaning = Meme {
        meaning: Some("used for a choice".into()),
        ..meme
    };
    let roundtrip: Meme =
        serde_json::from_value(serde_json::to_value(with_meaning).unwrap()).unwrap();
    assert_eq!(roundtrip.meaning.as_deref(), Some("used for a choice"));
}
