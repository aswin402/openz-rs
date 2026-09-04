//! Custom video-template CRUD handlers.

use super::{
    Json, McpObject, OpenMediaServer, Parameters, TemplateCreateRequest, TemplateDeleteRequest,
    TemplateReadRequest, TemplateUpdateRequest,
};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::template_router()
}

#[rmcp::tool_router(router = template_router)]
impl OpenMediaServer {
    #[rmcp::tool(
        name = "template_create",
        description = "Save a new custom video scene template. Custom templates can use {{parameter_name}} placeholders in their scene structure."
    )]
    pub async fn template_create(
        &self,
        params: Parameters<TemplateCreateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let mut req = params.0;
        req.parameter_schema = super::parse_json_value_field(&req.parameter_schema, "parameter_schema")?;
        req.scene_template = super::parse_json_value_field(&req.scene_template, "scene_template")?;
        let templates_dir = super::get_templates_dir();
        std::fs::create_dir_all(&templates_dir)
            .map_err(|e| format!("Failed to create templates directory: {}", e))?;

        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));

        let template_data = serde_json::json!({
            "name": req.name,
            "description": req.description,
            "parameter_schema": req.parameter_schema,
            "scene_template": req.scene_template,
        });

        let content = serde_json::to_string_pretty(&template_data).map_err(|e| e.to_string())?;

        std::fs::write(&template_path, content)
            .map_err(|e| format!("Failed to write template file: {}", e))?;

        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' created successfully", req.name),
            "path": template_path.to_string_lossy().to_string(),
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "template_read",
        description = "Read a specific custom template definition, or list all available custom and built-in templates."
    )]
    pub async fn template_read(
        &self,
        params: Parameters<TemplateReadRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = super::get_templates_dir();

        if let Some(ref name) = req.name {
            let template_path = templates_dir.join(format!("{}.json", name.to_lowercase()));
            if !template_path.exists() || !template_path.is_file() {
                return Err(format!("Template '{}' not found", name));
            }

            let s = std::fs::read_to_string(&template_path).map_err(|e| e.to_string())?;
            let custom_tmpl: serde_json::Value =
                serde_json::from_str(&s).map_err(|e| e.to_string())?;
            Ok(Json(McpObject(custom_tmpl)))
        } else {
            let mut list = Vec::new();

            list.push(serde_json::json!({
                "name": "slideshow",
                "type": "built-in",
                "description": "Generate slideshow video from an array of image files.",
                "expected_parameters": {
                    "images": "Array of image source paths/URLs",
                    "duration_per_image": "Float (default 3.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "text_explainer",
                "type": "built-in",
                "description": "Compile a text explainer video from bullet points and titles.",
                "expected_parameters": {
                    "title": "String",
                    "bullets": "Array of strings",
                    "bullet_duration": "Float (default 4.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "data_dashboard",
                "type": "built-in",
                "description": "Generate animated dashboard scenes showing statistical charts.",
                "expected_parameters": {
                    "title": "String",
                    "charts": "Array of chart configurations (type, title, data)",
                    "chart_duration": "Float (default 3.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "social_media",
                "type": "built-in",
                "description": "Create portrait videos (9:16 layout) with background and card animations for social media.",
                "expected_parameters": {
                    "title": "String",
                    "content": "Array of strings (points/facts)",
                    "scene_duration": "Float (default 3.0)",
                    "background_color": "String (default #1e1b4b)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "product_showcase",
                "type": "built-in",
                "description": "Generate video showcasing product image and descriptive features.",
                "expected_parameters": {
                    "product_name": "String",
                    "product_image": "String path",
                    "features": "Array of strings",
                    "scene_duration": "Float (default 3.0)",
                    "background_color": "String (default #111827)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));

            if templates_dir.exists() && templates_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&templates_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("json")
                        {
                            if let Ok(s) = std::fs::read_to_string(&path) {
                                if let Ok(custom_tmpl) =
                                    serde_json::from_str::<serde_json::Value>(&s)
                                {
                                    list.push(serde_json::json!({
                                        "name": custom_tmpl.get("name").unwrap_or(&serde_json::Value::Null),
                                        "type": "custom",
                                        "description": custom_tmpl.get("description").unwrap_or(&serde_json::Value::Null),
                                        "parameter_schema": custom_tmpl.get("parameter_schema").unwrap_or(&serde_json::Value::Null),
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            Ok(Json(McpObject(serde_json::json!({ "templates": list }))))
        }
    }

    #[rmcp::tool(
        name = "template_update",
        description = "Update an existing custom template definition."
    )]
    pub async fn template_update(
        &self,
        params: Parameters<TemplateUpdateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = super::get_templates_dir();
        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));

        if !template_path.exists() || !template_path.is_file() {
            return Err(format!("Template '{}' not found", req.name));
        }

        let s = std::fs::read_to_string(&template_path).map_err(|e| e.to_string())?;
        let mut template_data: serde_json::Value =
            serde_json::from_str(&s).map_err(|e| e.to_string())?;

        if let Some(desc) = req.description {
            template_data["description"] = serde_json::json!(desc);
        }
        if let Some(schema) = req.parameter_schema {
            template_data["parameter_schema"] =
                super::parse_json_value_field(&schema, "parameter_schema")?;
        }
        if let Some(template) = req.scene_template {
            template_data["scene_template"] =
                super::parse_json_value_field(&template, "scene_template")?;
        }

        let content = serde_json::to_string_pretty(&template_data).map_err(|e| e.to_string())?;

        std::fs::write(&template_path, content)
            .map_err(|e| format!("Failed to write updated template file: {}", e))?;

        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' updated successfully", req.name),
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "template_delete",
        description = "Delete a custom template definition."
    )]
    pub async fn template_delete(
        &self,
        params: Parameters<TemplateDeleteRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = super::get_templates_dir();
        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));

        if !template_path.exists() || !template_path.is_file() {
            return Err(format!("Template '{}' not found", req.name));
        }

        std::fs::remove_file(&template_path)
            .map_err(|e| format!("Failed to delete template file: {}", e))?;

        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' deleted successfully", req.name),
        });

        Ok(Json(McpObject(response)))
    }
}
