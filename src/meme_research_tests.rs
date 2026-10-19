use super::*;
use crate::research::SearchHit;
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Trimmed from a real GIPHY page (2026-10-09): meta tags plus embedded data
/// holding this GIF's record and a related GIF's tags.
const PAGE: &str = r#"<html><head>
<meta property="og:title" content="Tmkoc GIF - Find &amp; Share on GIPHY"/>
<meta property="og:image" content="https://media1.giphy.com/media/v1.Y2lkPTc5MGI3NjEx/SinIpWR8Jc6VVIdwNy/giphy.webp"/>
</head><body><script>self.__next_f.push([1,"{\"gifs\":[{\"id\":\"OTHER1\",\"rating\":\"pg\",\"tags\":[\"jethalal hairs\",\"tmkoc hairs\"]},{\"id\":\"SinIpWR8Jc6VVIdwNy\",\"rating\":\"g\",\"tags\":[\"giphycreatortest\",\"tmkoc\",\"jethalal\",\"chup ho ja satvi fail\"]}]}"])</script></body></html>"#;

const URL: &str = "https://giphy.com/gifs/tmkoc-jethalal-chup-ho-ja-satvi-fail-SinIpWR8Jc6VVIdwNy";

struct SearchHits(Vec<SearchHit>);

#[async_trait::async_trait]
impl WebSearch for SearchHits {
    async fn search(&self, _: &str, _: usize) -> Result<Vec<SearchHit>, BoxError> {
        Ok(self.0.clone())
    }
}

async fn mock_page_server(
    page_body: &'static str,
    page_status: &'static str,
    media_status: &'static str,
    media_type: &'static str,
    media_length: &'static str,
    expects_media: bool,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut page, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        let read = page.read(&mut request).await.unwrap();
        let mut requests = vec![String::from_utf8_lossy(&request[..read]).to_string()];
        let response = format!(
            "HTTP/1.1 {page_status}\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page_body}",
            page_body.len()
        );
        page.write_all(response.as_bytes()).await.unwrap();
        if expects_media {
            let (mut media, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 8192];
            let read = media.read(&mut request).await.unwrap();
            requests.push(String::from_utf8_lossy(&request[..read]).to_string());
            media
                .write_all(
                    format!(
                        "HTTP/1.1 {media_status}\r\ncontent-type: {media_type}\r\ncontent-length: {media_length}\r\nconnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
        requests
    });
    (format!("http://{address}/fixture"), task)
}

#[test]
fn gif_ids_come_only_from_single_gif_pages() {
    assert_eq!(giphy_gif_id(URL), Some("SinIpWR8Jc6VVIdwNy"));
    assert_eq!(giphy_gif_id("https://giphy.com/explore/indian"), None);
    assert_eq!(
        giphy_gif_id("https://tenor.com/search/indian-party-gifs"),
        None
    );
}

#[test]
fn a_real_page_yields_small_media_rating_tags_and_slug() {
    let found = parse_giphy_page(URL, PAGE).unwrap();
    assert_eq!(found.title, "Tmkoc GIF");
    assert_eq!(
        found.media_url,
        "https://media1.giphy.com/media/v1.Y2lkPTc5MGI3NjEx/SinIpWR8Jc6VVIdwNy/200.webp"
    );
    // This GIF's own record, not the related GIF listed first.
    assert_eq!(found.rating.as_deref(), Some("g"));
    assert!(found.tags.contains(&"chup ho ja satvi fail".to_owned()));
    assert!(!found.tags.contains(&"tmkoc hairs".to_owned()));
    assert_eq!(
        found.slug_words,
        ["tmkoc", "jethalal", "chup", "ho", "ja", "satvi", "fail"]
    );
}

#[test]
fn media_from_other_hosts_is_refused() {
    let page = PAGE.replace("media1.giphy.com", "evil.example");
    assert!(parse_giphy_page(URL, &page).is_none());
}

#[test]
fn page_parser_uses_fallback_meta_and_rejects_unusable_page_data() {
    assert_eq!(
        giphy_gif_id("https://www.giphy.com/gifs/dance-123?utm_source=x"),
        Some("123")
    );
    assert_eq!(giphy_gif_id("https://giphy.com/gifs/no-id-"), None);
    assert_eq!(giphy_gif_id("https://giphy.com/gifs/slug-bad_id"), None);

    let fallback = r#"<meta name="twitter:image" content="https://i.giphy.com/a.gif?x=1"><meta property="og:title" content="Fallback - Find &amp; Share on GIPHY"><script>{"rating":"PG-13","tags":["unrelated"]}</script>"#;
    let found = parse_giphy_page("https://giphy.com/gifs/slug-123", fallback).unwrap();
    assert_eq!(found.title, "Fallback");
    assert_eq!(found.media_url, "https://i.giphy.com/200.webp");
    assert_eq!(found.rating.as_deref(), Some("pg-13"));
    assert_eq!(found.tags, ["unrelated"]);

    assert!(parse_giphy_page(URL, "<html>missing image metadata</html>").is_none());
    let other_host = PAGE.replace("media1.giphy.com", "evil.example");
    assert!(parse_giphy_page(URL, &other_host).is_none());
}

#[tokio::test]
async fn page_researcher_fetches_a_gif_page_and_checks_the_small_media_headers() {
    let (endpoint, requests) =
        mock_page_server(PAGE, "200 OK", "200 OK", "image/webp", "2048", true).await;
    let search = Arc::new(SearchHits(vec![
        SearchHit {
            title: "valid".into(),
            url: URL.into(),
            snippet: None,
        },
        SearchHit {
            title: "not a gif page".into(),
            url: "https://giphy.com/explore/indian".into(),
            snippet: None,
        },
    ]));
    let mut researcher = GiphyPageResearcher::new(search, reqwest::Client::new());
    researcher.page_endpoint = Some(endpoint.clone());
    researcher.media_endpoint = Some(endpoint);
    let found = researcher.research("reaction").await.unwrap();
    assert_eq!(found, [parse_giphy_page(URL, PAGE).unwrap()]);
    let requests = requests.await.unwrap();
    assert!(requests[0].starts_with("GET /fixture HTTP/1.1"));
    assert!(requests[1].starts_with("HEAD /fixture HTTP/1.1"));
}

#[tokio::test]
async fn page_researcher_rejects_http_errors_and_oversized_or_non_image_media() {
    for (status, content_type, length, expected_head) in [
        ("200 OK", "text/html", "2048", true),
        ("200 OK", "image/gif", "1500001", true),
        ("503 Service Unavailable", "image/gif", "10", false),
    ] {
        let (endpoint, requests) = mock_page_server(
            PAGE,
            if expected_head { "200 OK" } else { status },
            status,
            content_type,
            length,
            expected_head,
        )
        .await;
        let search = Arc::new(SearchHits(vec![SearchHit {
            title: "candidate".into(),
            url: URL.into(),
            snippet: None,
        }]));
        let mut researcher = GiphyPageResearcher::new(search, reqwest::Client::new());
        researcher.page_endpoint = Some(endpoint.clone());
        researcher.media_endpoint = Some(endpoint);
        assert!(researcher.research("reaction").await.unwrap().is_empty());
        assert_eq!(
            requests.await.unwrap().len(),
            if expected_head { 2 } else { 1 }
        );
    }
}
