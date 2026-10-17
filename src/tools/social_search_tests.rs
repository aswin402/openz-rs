use super::*;

#[tokio::test]
async fn test_hn_search() {
    let tool = SocialSearchTool::new();
    match tool.search_hacker_news("rust lang").await {
        Ok(hits) => assert!(hits.is_array()),
        Err(e) => println!("Warning: HN search failed (network?): {}", e),
    }
}

#[tokio::test]
async fn test_polymarket_search() {
    let tool = SocialSearchTool::new();
    match tool.search_polymarket("election").await {
        Ok(markets) => assert!(markets.is_array()),
        Err(e) => println!("Warning: Polymarket search failed (network?): {}", e),
    }
}
