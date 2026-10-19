use super::*;
use std::sync::Mutex;

struct Hits(Mutex<Vec<String>>);

#[async_trait]
impl WebSearch for Hits {
    async fn search(&self, query: &str, _max: usize) -> Result<Vec<SearchHit>, BoxError> {
        self.0.lock().unwrap().push(query.to_owned());
        Ok(vec![SearchHit {
            title: "Hinglish slang guide".into(),
            url: "https://slang.example/guide/".into(),
            snippet: Some("bawaal: chaos; scene set hai: all arranged".into()),
        }])
    }
}

struct Model;

#[async_trait]
impl ChatModel for Model {
    async fn complete(&self, system: &str, _user: &str) -> Result<String, BoxError> {
        if system.starts_with("Turn the request") {
            return Ok("hinglish slang for job offer".into());
        }
        Ok(r#"{"terms": [
            {"term": "bawaal", "meaning": "chaos", "intensity": "spicy", "source_url": "https://slang.example/guide"},
            {"term": "made up", "meaning": "x", "intensity": "light", "source_url": "https://elsewhere.example/"}
        ]}"#
        .into())
    }
}

struct ScriptedSearch(Result<Vec<SearchHit>, &'static str>);

#[async_trait]
impl WebSearch for ScriptedSearch {
    async fn search(&self, _query: &str, _max: usize) -> Result<Vec<SearchHit>, BoxError> {
        self.0.clone().map_err(Into::into)
    }
}

struct ScriptedModel(&'static str);

#[async_trait]
impl ChatModel for ScriptedModel {
    async fn complete(&self, _system: &str, _user: &str) -> Result<String, BoxError> {
        Ok(self.0.to_owned())
    }
}

#[tokio::test]
async fn long_requests_are_condensed_and_terms_are_grounded_in_results() {
    let search = Arc::new(Hits(Mutex::new(Vec::new())));
    let r = SearchResearcher::new(search.clone(), Arc::new(Model));
    let request = format!(
        "Hinglish slang that fits this message: \"{}\"",
        "x".repeat(200)
    );
    let found = r.research(&request).await.unwrap();
    assert_eq!(
        search.0.lock().unwrap().as_slice(),
        ["hinglish slang for job offer"]
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].term, "bawaal");
    assert_eq!(found[0].min_tier, Tier::Spicy);
}

#[tokio::test]
async fn short_request_skips_condensing_and_empty_search_skips_extraction() {
    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Ok(Vec::new()))),
        Arc::new(ScriptedModel("not JSON")),
    );
    assert_eq!(format!("{r:?}"), "SearchResearcher { .. }");
    assert!(r.research("short query").await.unwrap().is_empty());
}

#[tokio::test]
async fn empty_condensed_query_and_search_failures_are_handled() {
    let long = "x".repeat(81);
    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Err("offline"))),
        Arc::new(ScriptedModel("\"\nignored\"")),
    );
    assert!(r.research(&long).await.unwrap().is_empty());

    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Err("offline"))),
        Arc::new(ScriptedModel("query")),
    );
    assert!(r.research("short").await.is_err());
}

#[tokio::test]
async fn malformed_extraction_is_reported_and_unknown_intensity_is_light() {
    let hits = vec![SearchHit {
        title: "Guide".into(),
        url: "https://slang.example/guide".into(),
        snippet: None,
    }];
    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Ok(hits.clone()))),
        Arc::new(ScriptedModel("no structured response")),
    );
    assert!(r.research("short").await.is_err());

    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Ok(hits))),
        Arc::new(ScriptedModel(
            r#"{"terms":[{"term":"yo","meaning":"hello","intensity":"new","source_url":"https://slang.example/guide"}]}"#,
        )),
    );
    let terms = r.research("short").await.unwrap();
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].min_tier, Tier::Light);
}

#[tokio::test]
async fn unhinged_research_intensity_is_preserved() {
    let hit = SearchHit {
        title: "Guide".into(),
        url: "https://slang.example/guide".into(),
        snippet: None,
    };
    let r = SearchResearcher::new(
        Arc::new(ScriptedSearch(Ok(vec![hit]))),
        Arc::new(ScriptedModel(
            r#"{"terms":[{"term":"wild","meaning":"very loud","intensity":"unhinged","source_url":"https://slang.example/guide"}]}"#,
        )),
    );
    let terms = r.research("short").await.unwrap();
    assert_eq!(terms[0].min_tier, Tier::Unhinged);
}
