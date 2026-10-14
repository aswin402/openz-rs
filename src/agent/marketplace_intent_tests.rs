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

#[test]
fn engineering_and_architecture_prompts_do_not_short_circuit() {
    assert_eq!(
        classify_marketplace_intent(
            "Write a 1-sentence technical definition of an AI agent marketplace architecture."
        ),
        MarketplaceIntent::NotMarketplace
    );
    assert!(clarification_question_for_marketplace_intent(
        "Build a landing page for an AI agent marketplace"
    )
    .is_none());
}
