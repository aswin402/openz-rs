use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

const MAX_TEMPLATE_DEPTH: usize = 10;

pub struct CompileTemplateTool;

fn render_template_string(template: &str, data: &Value) -> String {
    render_template_string_inner(template, data, 0)
}

fn render_template_string_inner(template: &str, data: &Value, depth: usize) -> String {
    if depth > MAX_TEMPLATE_DEPTH {
        tracing::warn!(
            "Template rendering exceeded max depth of {}",
            MAX_TEMPLATE_DEPTH
        );
        return template.to_string();
    }

    let mut result = template.to_string();

    // 1. Process loops of the form <!-- loop: key --> ... <!-- endloop -->
    loop {
        let loop_start_tag = "<!-- loop:";
        let loop_end_tag = "<!-- endloop -->";

        if let Some(start_idx) = result.find(loop_start_tag) {
            let start_tag_end = result[start_idx..].find("-->");
            if start_tag_end.is_none() {
                break;
            }
            let start_tag_end_idx = start_idx + start_tag_end.unwrap() + 3;

            // Extract the key name from <!-- loop: key -->
            let key_str = result[start_idx + loop_start_tag.len()..start_tag_end_idx - 3].trim();

            if let Some(end_idx) = result[start_tag_end_idx..].find(loop_end_tag) {
                let actual_end_idx = start_tag_end_idx + end_idx;
                let inner_template = &result[start_tag_end_idx..actual_end_idx];

                let mut loop_replacement = String::new();
                if let Some(arr) = data.get(key_str).and_then(|v| v.as_array()) {
                    for item in arr {
                        loop_replacement.push_str(&render_template_string_inner(
                            inner_template,
                            item,
                            depth + 1,
                        ));
                    }
                }

                let full_end_idx = actual_end_idx + loop_end_tag.len();
                result.replace_range(start_idx..full_end_idx, &loop_replacement);
            } else {
                break; // Unclosed loop tag
            }
        } else {
            break;
        }
    }

    // 2. Process scalar placeholders: {{ key }}
    if let Some(obj) = data.as_object() {
        for (k, v) in obj {
            let placeholder = format!("{{{{ {} }}}}", k);
            let placeholder_no_space = format!("{{{{{}}}}}", k);
            let val_str = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => v.to_string(),
            };
            result = result.replace(&placeholder, &val_str);
            result = result.replace(&placeholder_no_space, &val_str);
        }
    }

    result
}

#[async_trait::async_trait]
impl Tool for CompileTemplateTool {
    fn name(&self) -> &str {
        "compile_template"
    }

    fn description(&self) -> &str {
        "Compile structured JSON data into pre-designed HTML or PDF document/slide templates using placeholders and loops."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "template_path": {
                    "type": "string",
                    "description": "Absolute path to the HTML template file."
                },
                "data": {
                    "type": "object",
                    "description": "JSON object containing data to inject into placeholders (scalars or arrays for loops)."
                },
                "output_path": {
                    "type": "string",
                    "description": "Absolute path where the compiled document should be saved."
                },
                "output_format": {
                    "type": "string",
                    "enum": ["html", "pdf"],
                    "description": "Format of the output file. Default is auto-detected from output_path extension."
                }
            },
            "required": ["template_path", "data", "output_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let template_path_str = arguments
            .get("template_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'template_path' parameter"))?;
        let data = arguments
            .get("data")
            .ok_or_else(|| anyhow!("Missing 'data' parameter"))?;
        let output_path_str = arguments
            .get("output_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'output_path' parameter"))?;

        let template_path = crate::config::resolve_path(template_path_str);
        let output_path = crate::config::resolve_path(output_path_str);

        if !template_path.exists() {
            return Err(anyhow!("Template file not found at {:?}", template_path));
        }

        let template_content = fs::read_to_string(&template_path)
            .map_err(|e| anyhow!("Failed to read template file: {}", e))?;

        let rendered = render_template_string(&template_content, data);

        let output_format =
            if let Some(fmt) = arguments.get("output_format").and_then(|v| v.as_str()) {
                fmt.to_lowercase()
            } else {
                output_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("html")
                    .to_lowercase()
            };

        if output_format == "pdf" {
            // Write to a temporary HTML file in output directory
            let temp_html_path = output_path.with_extension("temp.html");
            fs::write(&temp_html_path, &rendered)
                .map_err(|e| anyhow!("Failed to write temporary HTML file: {}", e))?;

            // Navigate and print to PDF using gsd_browser
            let gsd_browser = crate::tools::gsd_browser::GsdBrowserTool;
            let temp_url = format!("file://{}", temp_html_path.to_string_lossy());

            // Navigate
            let nav_res = gsd_browser
                .call(&json!({
                    "action": "navigate",
                    "url": temp_url
                }))
                .await;

            if let Err(e) = nav_res {
                let _ = fs::remove_file(&temp_html_path);
                return Err(anyhow!(
                    "Failed to navigate browser to temporary page: {}",
                    e
                ));
            }

            // Save PDF
            let save_res = gsd_browser
                .call(&json!({
                    "action": "save_pdf",
                    "path": output_path.to_string_lossy()
                }))
                .await;

            // Cleanup temp file
            let _ = fs::remove_file(&temp_html_path);

            if let Err(e) = save_res {
                return Err(anyhow!("Failed to save page as PDF: {}", e));
            }

            Ok(json!({
                "status": "success",
                "message": format!("Successfully compiled template to PDF at {:?}", output_path)
            }))
        } else {
            // Write compiled HTML directly
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &rendered)
                .map_err(|e| anyhow!("Failed to write compiled document: {}", e))?;

            Ok(json!({
                "status": "success",
                "message": format!("Successfully compiled template to HTML at {:?}", output_path)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
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
}
