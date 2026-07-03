use opendoc_mcp::handlers::{docx, pptx, pdf};
use opendoc_mcp::handlers::load_to_ir;
use std::path::Path;

struct Topic {
    title: &'static str,
    points: &'static [&'static str],
}

const TOPICS: &[Topic] = &[
    Topic {
        title: "Introduction to Cryptography & Digital Cash",
        points: &[
            "Public-Key Cryptography secures asset ownership and transfers without intermediaries.",
            "SHA-256 cryptographic hash function guarantees block-level immutability.",
            "Distributed Ledger technology serves as a transparent, append-only ledger.",
            "Decentralized peer-to-peer validation replaces traditional trust authorities."
        ],
    },
    Topic {
        title: "The Genesis and History of Bitcoin (BTC)",
        points: &[
            "Satoshi Nakamoto released the seminal Bitcoin Whitepaper in October 2008.",
            "The Genesis Block was mined on January 3, 2009 with a historic message.",
            "Max supply is algorithmically capped at exactly 21,000,000 BTC.",
            "Halving cycles occur every 210,000 blocks (~4 years) to control inflation."
        ],
    },
    Topic {
        title: "How Bitcoin Mining and Consensus Works",
        points: &[
            "Miners solve compute-heavy hash puzzles to secure the blockchain.",
            "Proof of Work (PoW) consensus prevents double-spending and Sybil attacks.",
            "Difficulty adjustments dynamically maintain block generation at 10 minutes.",
            "Block rewards consist of block subsidies plus user transaction fees."
        ],
    },
    Topic {
        title: "Introduction to Cryptocurrency Trading",
        points: &[
            "Trading operates 24/7 across global centralized and decentralized exchanges.",
            "Order Books match bid and ask limit orders in real time.",
            "Liquidity is provided by automated market makers (AMMs) and institutional market makers.",
            "Spot markets trade physical coins; derivatives allow leverage and hedging."
        ],
    },
    Topic {
        title: "Technical Analysis (TA) Foundations",
        points: &[
            "Candlestick charts visualize price trends (Open, High, Low, Close) over time.",
            "Support levels act as buying floor; Resistance acts as selling ceiling.",
            "Trend lines define market direction: Uptrend, Downtrend, or Sideways range.",
            "Moving Averages (SMA/EMA) identify long-term macro trends."
        ],
    },
    Topic {
        title: "Advanced Trading Indicators & Volume Profile",
        points: &[
            "Relative Strength Index (RSI) identifies overbought and oversold conditions.",
            "MACD tracks momentum changes and signal line crossovers.",
            "Bollinger Bands measure market volatility using standard deviations.",
            "Volume profile identifies high-volume nodes and key price zones."
        ],
    },
    Topic {
        title: "Risk Management and Capital Preservation",
        points: &[
            "Capital preservation is the single most important rule for traders.",
            "Stop-Loss orders automatically exit losing trades at predetermined limits.",
            "Leverage increases purchasing power but accelerates liquidation risks.",
            "Asset diversification limits exposure to individual project failures."
        ],
    },
    Topic {
        title: "The Evolution of DeFi & Decentralized Apps",
        points: &[
            "Ethereum introduced smart contracts: self-executing software programs.",
            "Decentralized Finance (DeFi) offers lending, borrowing, and synthetic assets.",
            "Liquidity Pools allow users to earn yields by supplying digital assets.",
            "Layer-2 scaling networks (Rollups) reduce gas fees and latency."
        ],
    },
    Topic {
        title: "Institutional Adoption & Future Outlook",
        points: &[
            "Spot Bitcoin ETFs allow institutional capital to enter the space seamlessly.",
            "Lightning Network facilitates instant, microsecond microtransactions.",
            "Central Bank Digital Currencies (CBDCs) compete with private cryptos.",
            "Regulatory frameworks are evolving globally to establish compliance standards."
        ],
    },
];

fn main() {
    let scratch_dir = Path::new("scratch");
    std::fs::create_dir_all(scratch_dir).unwrap();

    let docx_path = scratch_dir.join("crypto_guide.docx");
    let pptx_path = scratch_dir.join("crypto_presentation.pptx");
    let pdf_path = scratch_dir.join("crypto_handbook.pdf");

    println!("==================================================");
    println!("GENERATING PREMIUM CRYPTO DOCUMENTS (10 PAGES EACH)");
    println!("==================================================");

    // ==========================================
    // 1. GENERATE DOCX (10 Pages)
    // ==========================================
    println!("Building DOCX...");
    docx::create_document(docx_path.to_str().unwrap(), Some("Bitcoin & Cryptocurrency Guide"));
    
    // Page 1: Title Cover
    docx::add_paragraph(
        docx_path.to_str().unwrap(),
        "THE CRYPTOCURRENCY HANDBOOK",
        Some(true),
        None,
        None,
        Some(28.0),
        Some("Georgia".to_string()),
        Some("F7931A".to_string()), // Bitcoin Gold
        None,
        Some("center".to_string()),
        None,
        Some(1.5),
        None,
        None,
        None,
    );
    docx::add_paragraph(
        docx_path.to_str().unwrap(),
        "A Comprehensive Guide to Bitcoin, Blockchain technology, DeFi, and Trading Strategies",
        None,
        Some(true),
        None,
        Some(14.0),
        Some("Arial".to_string()),
        Some("555555".to_string()),
        None,
        Some("center".to_string()),
        None,
        Some(1.2),
        None,
        None,
        None,
    );
    docx::add_paragraph(
        docx_path.to_str().unwrap(),
        "Published by Opendoc Engine • 2026 Edition",
        None,
        None,
        None,
        Some(11.0),
        Some("Arial".to_string()),
        Some("888888".to_string()),
        None,
        Some("center".to_string()),
        None,
        Some(1.0),
        None,
        None,
        None,
    );

    // Pages 2 to 10: Topics
    for (i, topic) in TOPICS.iter().enumerate() {
        // Page break before
        docx::add_paragraph(
            docx_path.to_str().unwrap(),
            &format!("Topic {}: {}", i + 2, topic.title),
            Some(true),
            None,
            None,
            Some(18.0),
            Some("Georgia".to_string()),
            Some("0D1B2A".to_string()), // Deep Navy
            None,
            Some("left".to_string()),
            None,
            Some(1.3),
            None,
            None,
            Some(true), // Page break
        );

        for point in topic.points {
            docx::add_paragraph(
                docx_path.to_str().unwrap(),
                &format!("• {}", point),
                None,
                None,
                None,
                Some(11.0),
                Some("Arial".to_string()),
                Some("333333".to_string()),
                None,
                Some("left".to_string()),
                None,
                Some(1.15),
                None,
                None,
                None,
            );
        }

        // Add table comparison on page 5 (Introduction to Cryptocurrency Trading)
        if i == 3 {
            docx::add_paragraph(
                docx_path.to_str().unwrap(),
                "Market Comparison Matrix:",
                Some(true),
                None,
                None,
                Some(12.0),
                Some("Georgia".to_string()),
                Some("F7931A".to_string()),
                None,
                Some("left".to_string()),
                None,
                None,
                None,
                None,
                None,
            );
            docx::add_table(
                docx_path.to_str().unwrap(),
                &["Market Type".to_string(), "Ownership".to_string(), "Leverage".to_string(), "Settlement".to_string()],
                &[
                    vec!["Spot Market".to_string(), "Immediate Asset Delivery".to_string(), "None (1x)".to_string(), "Instant (T+0)".to_string()],
                    vec!["Futures Market".to_string(), "Contract Representation".to_string(), "Up to 100x".to_string(), "Expiry/Daily".to_string()],
                    vec!["Options Market".to_string(), "Right, Not Obligation".to_string(), "Premium Only".to_string(), "Expiry".to_string()]
                ],
                Some(100.0),
                Some("center".to_string()),
                Some("single".to_string()),
                Some(4),
                Some("CCCCCC".to_string()),
                Some("F7931A".to_string()),
                Some("F5F5F5".to_string()),
                Some(true),
            );
        }
    }

    // ==========================================
    // 2. GENERATE PPTX (10 Slides)
    // ==========================================
    println!("Building PPTX...");
    pptx::create_presentation(pptx_path.to_str().unwrap(), Some("Bitcoin & Cryptocurrencies"));
    
    // Slide 1 is created automatically. Add 9 slides (Slides 2 to 10)
    for (i, topic) in TOPICS.iter().enumerate() {
        let body_items: Vec<String> = topic.points.iter().map(|p| p.to_string()).collect();
        pptx::add_slide(
            pptx_path.to_str().unwrap(),
            &format!("Topic {}: {}", i + 2, topic.title),
            Some(&body_items),
            Some("1A1A2E".to_string()), // Deep navy space background
            Some(24.0),
            Some("FFFFFF".to_string()), // White text
            Some("Segoe UI".to_string()),
            Some("left".to_string()),
        );
    }

    // ==========================================
    // 3. GENERATE PDF (10 Pages)
    // ==========================================
    println!("Building PDF...");
    let mut pdf_text = String::new();
    
    // Page 1: Cover
    pdf_text.push_str("THE CRYPTOCURRENCY HANDBOOK\n");
    pdf_text.push_str("A Comprehensive Guide to Bitcoin, Blockchain, DeFi, and Trading Strategies\n");
    pdf_text.push_str("Published by Opendoc Engine • 2026 Edition\n");

    // Pages 2 to 10: Topics
    for (i, topic) in TOPICS.iter().enumerate() {
        pdf_text.push('\x0c'); // Explicit page break
        pdf_text.push_str(&format!("TOPIC {}: {}\n\n", i + 2, topic.title.to_uppercase()));
        for point in topic.points {
            pdf_text.push_str(&format!("• {}\n\n", point));
        }
    }

    let pdf_config = pdf::PdfLayoutConfig {
        title: Some("Cryptocurrency Handbook".to_string()),
        author: Some("Opendoc Engine".to_string()),
        page_numbers: true,
        ..pdf::PdfLayoutConfig::default()
    };
    pdf::create_formatted_pdf(pdf_path.to_str().unwrap(), &pdf_text, &pdf_config);

    // ==========================================
    // VERIFY ALL DOCUMENTS
    // ==========================================
    println!("\n=== VERIFYING CRYPTO DOCUMENTS ===");

    // Verify DOCX
    let docx_ir = load_to_ir(docx_path.to_str().unwrap()).unwrap();
    println!("✔ DOCX successfully parsed. Format: {}, Paragraphs: {}", docx_ir.format, docx_ir.paragraphs.len());
    // 3 initial + (9 topics * 4 paragraphs/topic) + headings/table titles = should be > 40
    assert!(docx_ir.paragraphs.len() > 30);

    // Verify PPTX
    let pptx_ir = load_to_ir(pptx_path.to_str().unwrap()).unwrap();
    println!("✔ PPTX successfully parsed. Format: {}, Slides: {}", pptx_ir.format, pptx_ir.metadata.page_count.unwrap());
    assert_eq!(pptx_ir.metadata.page_count.unwrap(), 10);

    // Verify PDF
    let pdf_ir = load_to_ir(pdf_path.to_str().unwrap()).unwrap();
    println!("✔ PDF successfully parsed. Format: {}, Pages: {}", pdf_ir.format, pdf_ir.metadata.page_count.unwrap());
    assert_eq!(pdf_ir.metadata.page_count.unwrap(), 10);

    println!("\n==================================================");
    println!("SUCCESS: ALL CRYPTO DOCUMENTS GENERATED AND VERIFIED!");
    println!("==================================================");
}
