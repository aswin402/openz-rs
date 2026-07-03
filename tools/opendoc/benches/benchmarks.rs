use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_docx_to_ir(c: &mut Criterion) {
    let dir = std::env::temp_dir();
    let path = dir.join("bench_docx.docx");
    let p = path.to_str().unwrap();

    // Create a test DOCX
    let mut doc = rdocx::Document::new();
    doc.save(p).unwrap();

    c.bench_function("docx_to_ir", |b| {
        b.iter(|| {
            let _ = opendoc_mcp::handlers::docx::to_ir(black_box(p));
        })
    });

    let _ = std::fs::remove_file(path);
}

fn bench_load_to_ir(c: &mut Criterion) {
    let dir = std::env::temp_dir();
    let path = dir.join("bench_txt.txt");
    let p = path.to_str().unwrap();
    std::fs::write(p, "Hello, World!").unwrap();

    c.bench_function("load_txt_to_ir", |b| {
        b.iter(|| {
            let _ = opendoc_mcp::handlers::load_to_ir(black_box(p));
        })
    });

    let _ = std::fs::remove_file(path);
}

fn bench_search(c: &mut Criterion) {
    use opendoc_mcp::engine::search;
    use opendoc_mcp::ir::Document;

    let mut doc = Document::new("txt");
    for i in 0..100 {
        doc.paragraphs
            .push(opendoc_mcp::ir::Paragraph::new(format!("Paragraph {}", i)));
    }

    c.bench_function("search_100_paragraphs", |b| {
        b.iter(|| {
            let _ = search::search_document(black_box(&doc), black_box("Paragraph"), false);
        })
    });
}

fn bench_pdf_text_extraction(c: &mut Criterion) {
    let dir = std::env::temp_dir();
    let path = dir.join("bench_pdf_extract.pdf");
    let p = path.to_str().unwrap();

    let _ = opendoc_mcp::handlers::pdf::create_pdf(p, "Page 1 content\x0cPage 2 content\x0cPage 3 content", None);

    c.bench_function("pdf_text_extraction", |b| {
        b.iter(|| {
            let doc = lopdf::Document::load(p).unwrap();
            let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
            let _ = doc.extract_text(&pages);
        })
    });

    let _ = std::fs::remove_file(path);
}

fn bench_template_rendering(c: &mut Criterion) {
    use opendoc_mcp::engine::template::fill_template_enhanced;
    use opendoc_mcp::ir::Document;
    use serde_json::json;

    let data = json!({
        "user": {
            "name": "Alice"
        },
        "site_name": "Opendoc",
        "items": ["a", "b", "c", "d", "e"]
    });

    c.bench_function("template_rendering_complex", |b| {
        b.iter_with_setup(
            || {
                let mut doc = Document::new("txt");
                doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Hello {{user.name}}! Welcome to {{site_name}}."));
                for _ in 0..5 {
                    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Item: {{#items}}{{this}} {{/items}}"));
                }
                doc
            },
            |mut doc| {
                let _ = fill_template_enhanced(&mut doc, &data);
            }
        )
    });
}

fn bench_pdf_layout_creation(c: &mut Criterion) {
    let dir = std::env::temp_dir();
    let path = dir.join("bench_pdf_layout.pdf");
    let p = path.to_str().unwrap();

    let text = "This is line 1 of our layout.\n\nThis is line 2 of our layout.\n\nHere is some longer text to test the automatic word-wrap and boundary checking performance in the PDF layout generation algorithm.";

    c.bench_function("pdf_layout_creation", |b| {
        b.iter(|| {
            let _ = opendoc_mcp::handlers::pdf::create_pdf(p, text, None);
        })
    });

    let _ = std::fs::remove_file(path);
}

fn bench_docx_image_extraction(c: &mut Criterion) {
    let dir = std::env::temp_dir();
    let path_docx = dir.join("bench_extract.docx");
    let path_out = dir.join("bench_extract_out");
    let p_docx = path_docx.to_str().unwrap();
    let p_out = path_out.to_str().unwrap();

    let mut doc = rdocx::Document::new();
    doc.save(p_docx).unwrap();
    let _ = std::fs::create_dir_all(&path_out);

    c.bench_function("docx_image_extraction", |b| {
        b.iter(|| {
            let _ = opendoc_mcp::handlers::extract_images_from_zip(black_box(p_docx), black_box(p_out));
        })
    });

    let _ = std::fs::remove_file(path_docx);
    let _ = std::fs::remove_dir_all(path_out);
}

criterion_group!(
    benches,
    bench_docx_to_ir,
    bench_load_to_ir,
    bench_search,
    bench_pdf_text_extraction,
    bench_template_rendering,
    bench_pdf_layout_creation,
    bench_docx_image_extraction
);
criterion_main!(benches);
