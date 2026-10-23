#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceIntent {
    Buyer,
    Seller,
    AmbiguousBuySell,
    NotMarketplace,
}

const BUYER_KEYWORDS: &[&str] = &[
    "buy",
    "buyer",
    "purchase",
    "where can i get",
    "where to get",
    "buyer-side",
    "buyer side",
    "buy-side",
    "customer-side",
    "ready-made",
    "ready made",
    "use ready",
    "agents to use",
    "get agents",
];

const SELLER_KEYWORDS: &[&str] = &[
    "seller-side",
    "seller side",
    "sell-side",
    "creator-side",
    "sell my",
    "sell our",
    "sell agents",
    "sell ai agents",
    "selling my",
    "selling our",
    "monetize",
    "creator payout",
    "revenue share",
    "publish my",
    "publish our",
];

const AMBIGUOUS_MARKETPLACE_KEYWORDS: &[&str] = &[
    "selling platform",
    "selling platforms",
    "agent marketplace",
    "agent marketplaces",
];

const ENGINEERING_KEYWORDS: &[&str] = &[
    "write ",
    "build ",
    "create ",
    "design ",
    "implement ",
    "code ",
    "define ",
    "definition",
    "explain ",
    "architecture",
    "schema",
    "landing page",
    "html",
    "css",
    "database",
    "api",
];

const MARKETPLACE_NOUN_KEYWORDS: &[&str] = &[
    "platform",
    "platforms",
    "marketplace",
    "marketplaces",
    "store",
    "stores",
    "directory",
    "directories",
];

const AI_AGENT_KEYWORDS: &[&str] = &["ai agent", "ai agents", "agent", "agents"];

pub fn classify_marketplace_intent(text: &str) -> MarketplaceIntent {
    let lower = text.to_lowercase();
    match !mentions_ai_agent(&lower) || is_engineering_or_explanatory_prompt(&lower) {
        true => return MarketplaceIntent::NotMarketplace,
        false => {}
    }

    let buyer = contains_any(&lower, BUYER_KEYWORDS);
    let seller = contains_any(&lower, SELLER_KEYWORDS);
    let ambiguous = mentions_agent_marketplace(&lower)
        && contains_any(&lower, AMBIGUOUS_MARKETPLACE_KEYWORDS);

    let rules: [(bool, MarketplaceIntent); 3] = [
        (buyer && !seller, MarketplaceIntent::Buyer),
        (seller && !buyer, MarketplaceIntent::Seller),
        (ambiguous, MarketplaceIntent::AmbiguousBuySell),
    ];
    rules
        .into_iter()
        .find_map(|(matched, intent)| matched.then_some(intent))
        .unwrap_or(MarketplaceIntent::NotMarketplace)
}

fn is_engineering_or_explanatory_prompt(lower: &str) -> bool {
    contains_any(lower, ENGINEERING_KEYWORDS)
}

pub fn clarification_question_for_marketplace_intent(text: &str) -> Option<&'static str> {
    match classify_marketplace_intent(text) {
        MarketplaceIntent::AmbiguousBuySell => Some(
            "Do you mean platforms where you can buy/use ready-made AI agents, or platforms where creators can sell/publish AI agents?",
        ),
        _ => None,
    }
}

fn mentions_agent_marketplace(lower: &str) -> bool {
    mentions_ai_agent(lower) && contains_any(lower, MARKETPLACE_NOUN_KEYWORDS)
}

fn mentions_ai_agent(lower: &str) -> bool {
    contains_any(lower, AI_AGENT_KEYWORDS)
}

use crate::tools::routing::contains_any;

#[cfg(test)]
#[path = "marketplace_intent_tests.rs"]
mod tests;
