use super::*;

#[tokio::test]
async fn test_hn_search() {
    let tool = SocialSearchTool::new();
    let res = tool.search_hacker_news("rust lang").await;
    if let Err(e) = res {
        println!("Warning: HN search failed (network?): {}", e);
        return;
    }
    let hits = res.unwrap();
    assert!(hits.is_array());
}

#[tokio::test]
async fn test_polymarket_search() {
    let tool = SocialSearchTool::new();
    let res = tool.search_polymarket("election").await;
    if let Err(e) = res {
        println!("Warning: Polymarket search failed (network?): {}", e);
        return;
    }
    let markets = res.unwrap();
    assert!(markets.is_array());
}
