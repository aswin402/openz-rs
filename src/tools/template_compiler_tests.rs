use super::*;

#[test]
fn test_render_template_string_simple() {
    let template = "Hello {{ name }}, you have {{ count }} tasks.";
    let data = json!({
        "name": "Aswin",
        "count": 5
    });
    let res = render_template_string(template, &data);
    assert_eq!(res, "Hello Aswin, you have 5 tasks.");
}

#[test]
fn test_render_template_string_loop() {
    let template = "Header\n<!-- loop: items -->Item: {{ name }}\n<!-- endloop -->Footer";
    let data = json!({
        "items": [
            { "name": "Task 1" },
            { "name": "Task 2" }
        ]
    });
    let res = render_template_string(template, &data);
    assert_eq!(res, "Header\nItem: Task 1\nItem: Task 2\nFooter");
}

#[tokio::test]
async fn test_compile_template_tool_html() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_temp_compiler_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;

    let template_path = temp_dir.join("template.html");
    fs::write(&template_path, "Title: {{ title }}, Author: {{ author }}")?;

    let output_path = temp_dir.join("output.html");

    let tool = CompileTemplateTool;
    let args = json!({
        "template_path": template_path.to_string_lossy().to_string(),
        "data": {
            "title": "OpenZ Guide",
            "author": "Google DeepMind"
        },
        "output_path": output_path.to_string_lossy().to_string(),
        "output_format": "html"
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_path.exists());

    let content = fs::read_to_string(&output_path)?;
    assert_eq!(content, "Title: OpenZ Guide, Author: Google DeepMind");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
