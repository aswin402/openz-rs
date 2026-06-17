use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

pub struct MermaidRendererTool;

#[async_trait::async_trait]
impl Tool for MermaidRendererTool {
    fn name(&self) -> &str {
        "render_mermaid"
    }

    fn description(&self) -> &str {
        "Render a Mermaid diagram (flowchart, sequence diagram, class diagram, etc.) into an SVG image file."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "chart": {
                    "type": "string",
                    "description": "The raw Mermaid diagram code (e.g. 'flowchart TD\\n  A --> B')."
                },
                "output_path": {
                    "type": "string",
                    "description": "The file path to save the generated SVG diagram (defaults to 'diagram.svg' in the current workspace)."
                }
            },
            "required": ["chart"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let chart = arguments.get("chart").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'chart' parameter"))?;
        
        let output_path_str = arguments.get("output_path").and_then(|v| v.as_str()).unwrap_or("diagram.svg");
        let output_path = crate::config::resolve_path(output_path_str);

        // Run rendering using the pure Rust mermaid-rs-renderer
        let svg = mermaid_rs_renderer::render(chart)
            .map_err(|e| anyhow!("Mermaid rendering failed: {}", e))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, &svg)?;

        Ok(json!({
            "status": "success",
            "output_path": output_path.to_string_lossy(),
            "message": format!("Mermaid diagram successfully rendered and saved to '{}'.", output_path_str)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_render_mermaid() -> Result<()> {
        let tool = MermaidRendererTool;
        let temp_dir = std::env::temp_dir().join(format!("openz_mermaid_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;
        let output_file = temp_dir.join("test_chart.svg");

        let args = json!({
            "chart": "flowchart LR\n  A --> B",
            "output_path": output_file.to_str().unwrap()
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        assert!(output_file.exists());

        let content = fs::read_to_string(output_file)?;
        assert!(content.contains("<svg"));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
