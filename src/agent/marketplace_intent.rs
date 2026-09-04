#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceIntent {
    Buyer,
    Seller,
    AmbiguousBuySell,
    NotMarketplace,
}

pub fn classify_marketplace_intent(text: &str) -> MarketplaceIntent {
    let lower = text.to_lowercase();
    if !mentions_ai_agent(&lower) {
        return MarketplaceIntent::NotMarketplace;
    }

    let buyer = contains_any(
        &lower,
        &[
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
        ],
    );
    let seller = contains_any(
        &lower,
        &[
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
        ],
    );

    if buyer && !seller {
        MarketplaceIntent::Buyer
    } else if seller && !buyer {
        MarketplaceIntent::Seller
    } else if mentions_agent_marketplace(&lower)
        && (lower.contains("selling platform")
            || lower.contains("selling platforms")
            || lower.contains("agent marketplace")
            || lower.contains("agent marketplaces"))
    {
        MarketplaceIntent::AmbiguousBuySell
    } else {
        MarketplaceIntent::NotMarketplace
    }
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
    mentions_ai_agent(lower)
        && contains_any(
            lower,
            &[
                "platform",
                "platforms",
                "marketplace",
                "marketplaces",
                "store",
                "stores",
                "directory",
                "directories",
            ],
        )
}

fn mentions_ai_agent(lower: &str) -> bool {
    contains_any(lower, &["ai agent", "ai agents", "agent", "agents"])
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_sell_marketplace_language_is_ambiguous() {
        assert_eq!(
            classify_marketplace_intent(
                "research ai agents selling platforms and best marketplace"
            ),
            MarketplaceIntent::AmbiguousBuySell
        );
        assert!(
            clarification_question_for_marketplace_intent("ai agent selling platforms")
                .expect("clarification")
                .contains("buy/use ready-made")
        );
    }

    #[test]
    fn unambiguous_buy_marketplace_request_does_not_clarify() {
        assert_eq!(
            classify_marketplace_intent(
                "platform where we can buy ai agents for crypto and finance"
            ),
            MarketplaceIntent::Buyer
        );
        assert!(clarification_question_for_marketplace_intent(
            "platform where we can buy ai agents"
        )
        .is_none());
    }

    #[test]
    fn explicit_buyer_side_marketplace_label_does_not_clarify() {
        assert_eq!(
            classify_marketplace_intent(
                "Research buyer-side AI agent marketplaces where customers buy or subscribe to ready-made agents"
            ),
            MarketplaceIntent::Buyer
        );
        assert!(clarification_question_for_marketplace_intent(
            "Research buyer-side AI agent marketplaces where customers buy agents"
        )
        .is_none());
    }

    #[test]
    fn explicit_seller_side_marketplace_label_does_not_clarify() {
        assert_eq!(
            classify_marketplace_intent(
                "Research seller-side AI agent monetization platforms where creators sell or publish agents"
            ),
            MarketplaceIntent::Seller
        );
        assert!(clarification_question_for_marketplace_intent(
            "Research seller-side AI agent monetization platforms"
        )
        .is_none());
    }

    #[test]
    fn unambiguous_sell_marketplace_request_does_not_clarify() {
        assert_eq!(
            classify_marketplace_intent("where can I sell my AI agents and get creator payout"),
            MarketplaceIntent::Seller
        );
        assert!(
            clarification_question_for_marketplace_intent("where can I sell my AI agents")
                .is_none()
        );
    }
}
