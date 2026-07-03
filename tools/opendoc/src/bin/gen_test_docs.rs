use opendoc_mcp::handlers::{docx, pptx, pdf};
use opendoc_mcp::handlers::load_to_ir;
use std::path::Path;

fn main() {
    let scratch_dir = Path::new("scratch");
    std::fs::create_dir_all(scratch_dir).unwrap();

    let docx_path = scratch_dir.join("test_20page.docx");
    let pdf_path = scratch_dir.join("test_20page.pdf");
    let pptx_path = scratch_dir.join("test_8page.pptx");

    println!("Generating DOCX document (20 pages)...");
    // 1. Create DOCX
    docx::create_document(docx_path.to_str().unwrap(), Some("20 Page DOCX Test Document"));
    for i in 1..=20 {
        docx::add_paragraph(
            docx_path.to_str().unwrap(),
            &format!("This is page {} of the 20-page Word document.", i),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(i > 1), // Page break before on all pages after page 1
        );
    }

    println!("Generating PPTX document (8 pages)...");
    // 2. Create PPTX (initializes with 1 title slide)
    pptx::create_presentation(pptx_path.to_str().unwrap(), Some("8 Slide Presentation"));
    let body_text = vec!["Point A".to_string(), "Point B".to_string()];
    // Add 7 content slides to make it exactly 8 slides/pages total
    for i in 2..=8 {
        pptx::add_slide(
            pptx_path.to_str().unwrap(),
            &format!("Slide Title {}", i),
            Some(&body_text),
            None,
            None,
            None,
            None,
            None,
        );
    }

    println!("Generating PDF document (20 pages)...");
    // 3. Create PDF
    let mut pdf_text = String::new();
    for i in 1..=20 {
        if i > 1 {
            pdf_text.push('\x0c');
        }
        pdf_text.push_str(&format!("This is page {} of the 20-page PDF document.\n", i));
    }
    pdf::create_pdf(pdf_path.to_str().unwrap(), &pdf_text, Some("Agent Tester"));

    println!("All documents generated successfully!");

    // Verification
    println!("\n=== Verifying Generated Documents ===");

    // Verify DOCX
    let docx_ir = load_to_ir(docx_path.to_str().unwrap()).unwrap();
    println!("DOCX loaded successfully. Type: {}, Paragraphs count: {}", docx_ir.format, docx_ir.paragraphs.len());
    // Expect 21 paragraphs (1 default empty paragraph from template + 20 added paragraphs)
    assert_eq!(docx_ir.paragraphs.len(), 21);

    // Verify PPTX
    let pptx_ir = load_to_ir(pptx_path.to_str().unwrap()).unwrap();
    println!("PPTX loaded successfully. Type: {}, Slide count in metadata: {}", pptx_ir.format, pptx_ir.metadata.page_count.unwrap_or(0));
    assert_eq!(pptx_ir.metadata.page_count.unwrap_or(0), 8);

    // Verify PDF
    let pdf_ir = load_to_ir(pdf_path.to_str().unwrap()).unwrap();
    println!("PDF loaded successfully. Type: {}, Page count in metadata: {}, Paragraphs count: {}", pdf_ir.format, pdf_ir.metadata.page_count.unwrap_or(0), pdf_ir.paragraphs.len());
    assert_eq!(pdf_ir.metadata.page_count.unwrap_or(0), 20);
    assert!(pdf_ir.paragraphs.len() >= 20);

    // Real low-level page count verification for PDF
    let doc = lopdf::Document::load(pdf_path.to_str().unwrap()).unwrap();
    println!("PDF real page count via lopdf: {}", doc.get_pages().len());
    assert_eq!(doc.get_pages().len(), 20);

    println!("\n=== ALL VERIFICATIONS PASSED SUCCESSFULLY! ===");
}
