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
    match depth > MAX_TEMPLATE_DEPTH {
        true => {
            tracing::warn!(
                "Template rendering exceeded max depth of {}",
                MAX_TEMPLATE_DEPTH
            );
            return template.to_string();
        }
        false => {}
    }

    let mut result = template.to_string();

    // 1. Process loops of the form <!-- loop: key --> ... <!-- endloop -->
    let loop_start_tag = "<!-- loop:";
    let loop_end_tag = "<!-- endloop -->";
    while let Some((start_idx, start_tag_end_idx, actual_end_idx)) =
        result.find(loop_start_tag).and_then(|start_idx| {
            let start_tag_end = result[start_idx..].find("-->")?;
            let start_tag_end_idx = start_idx + start_tag_end + 3;
            let end_idx = result[start_tag_end_idx..].find(loop_end_tag)?;
            Some((start_idx, start_tag_end_idx, start_tag_end_idx + end_idx))
        })
    {
        let key_str = result[start_idx + loop_start_tag.len()..start_tag_end_idx - 3].trim();
        let inner_template = &result[start_tag_end_idx..actual_end_idx];

        let loop_replacement: String = data
            .get(key_str)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .map(|item| render_template_string_inner(inner_template, item, depth + 1))
            .collect();

        let full_end_idx = actual_end_idx + loop_end_tag.len();
        result.replace_range(start_idx..full_end_idx, &loop_replacement);
    }

    // 2. Process scalar placeholders: {{ key }}
    for (k, v) in data.as_object().into_iter().flatten() {
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

        template_path
            .exists()
            .then_some(())
            .ok_or_else(|| anyhow!("Template file not found at {:?}", template_path))?;

        let template_content = fs::read_to_string(&template_path)
            .map_err(|e| anyhow!("Failed to read template file: {}", e))?;

        let rendered = render_template_string(&template_content, data);

        let output_format = arguments
            .get("output_format")
            .and_then(|v| v.as_str())
            .or_else(|| output_path.extension().and_then(|ext| ext.to_str()))
            .unwrap_or("html")
            .to_lowercase();

        match output_format.as_str() {
            "pdf" => {
                // Write to a temporary HTML file in output directory
                let temp_html_path = output_path.with_extension("temp.html");
                fs::write(&temp_html_path, &rendered)
                    .map_err(|e| anyhow!("Failed to write temporary HTML file: {}", e))?;

                // Navigate and print to PDF using gsd_browser
                let gsd_browser = crate::tools::gsd_browser::GsdBrowserTool;
                let temp_url = format!("file://{}", temp_html_path.to_string_lossy());

                let nav_res = gsd_browser
                    .call(&json!({
                        "action": "navigate",
                        "url": temp_url
                    }))
                    .await;

                match nav_res {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = fs::remove_file(&temp_html_path);
                        return Err(anyhow!(
                            "Failed to navigate browser to temporary page: {}",
                            e
                        ));
                    }
                }

                let save_res = gsd_browser
                    .call(&json!({
                        "action": "save_pdf",
                        "path": output_path.to_string_lossy()
                    }))
                    .await;

                let _ = fs::remove_file(&temp_html_path);

                save_res.map_err(|e| anyhow!("Failed to save page as PDF: {}", e))?;

                Ok(json!({
                    "status": "success",
                    "message": format!("Successfully compiled template to PDF at {:?}", output_path)
                }))
            }
            _ => {
                match output_path.parent() {
                    Some(parent) => fs::create_dir_all(parent)?,
                    None => {}
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
}

#[cfg(test)]
#[path = "template_compiler_tests.rs"]
mod tests;
