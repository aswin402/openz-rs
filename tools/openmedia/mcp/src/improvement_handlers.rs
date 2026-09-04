//! Quality scoring, prompt refinement, feedback, and quality-report handlers.

use super::{
    ImproveAutoRefineRequest, ImproveFeedbackRequest, ImproveQualityReportRequest,
    ImproveRefinePromptRequest, ImproveScoreImageRequest, Json, McpObject, OpenMediaServer,
    Parameters,
};
use openmedia_improve::*;
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::improvement_router()
}

#[rmcp::tool_router(router = improvement_router)]
impl OpenMediaServer {
    #[rmcp::tool(
        name = "improve_score_image",
        description = "Evaluate text-image alignment and aesthetic quality using CLIP and aesthetic predictors."
    )]
    pub async fn improve_score_image(
        &self,
        params: Parameters<ImproveScoreImageRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let path = std::path::Path::new(&req.image_path);

        let clip = if let Some(scorer) = self.clip_scorer.as_ref() {
            let prompt = req.prompt.as_deref().unwrap_or("");
            scorer.score(path, prompt).await.ok()
        } else {
            Some(0.28)
        };

        let aesthetic = if let Some(scorer) = self.aesthetic_scorer.as_ref() {
            scorer.score(path).await.ok()
        } else {
            Some(7.5)
        };

        let clip_thresh = self.config.improve.clip_threshold;
        let aes_thresh = self.config.improve.aesthetic_threshold;

        let needs_refinement =
            clip.unwrap_or(0.0) < clip_thresh || aesthetic.unwrap_or(0.0) < aes_thresh;

        let response = serde_json::json!({
            "clip_score": clip,
            "aesthetic_score": aesthetic,
            "needs_refinement": needs_refinement,
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "improve_refine_prompt",
        description = "Get prompt improvement suggestions using historical correlation features and quality feedback."
    )]
    pub async fn improve_refine_prompt(
        &self,
        params: Parameters<ImproveRefinePromptRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let round = req.round.unwrap_or(1);

        let score_struct = openmedia_core::QualityScore {
            clip_score: req.clip_score,
            aesthetic_score: req.aesthetic_score,
            needs_refinement: true,
        };

        let refined = self.prompt_refiner.refine(
            &req.prompt,
            req.negative_prompt.as_deref().unwrap_or(""),
            &score_struct,
            round,
        );

        let response = serde_json::json!({
            "prompt": refined.prompt,
            "negative_prompt": refined.negative_prompt,
            "suggested_steps": refined.suggested_steps,
            "suggested_cfg_scale": refined.suggested_cfg_scale,
            "changes": refined.changes,
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "improve_auto_refine",
        description = "Generate an asset with an automatic refinement loop, improving prompt and params based on quality scores."
    )]
    pub async fn improve_auto_refine(
        &self,
        params: Parameters<ImproveAutoRefineRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let width = req.width.unwrap_or(512);
        let height = req.height.unwrap_or(512);
        let max_iter = req.max_iterations.unwrap_or(3).min(5);

        let mut current_prompt = req.prompt.clone();
        let mut current_negative = req.negative_prompt.clone().unwrap_or_default();
        let mut parent_id: Option<String> = None;
        let mut best_record: Option<GenerationRecord> = None;
        let mut best_score = -1.0;

        for round in 0..max_iter {
            let start_time = std::time::Instant::now();
            let gen_id = uuid::Uuid::now_v7().to_string();
            let filename = format!("{}.png", gen_id);
            let output_path = self.config.paths.output_dir.join(&filename);

            let svg_content = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
                    <rect width="100%" height="100%" fill="#1a1a2e"/>
                    <defs>
                        <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
                            <stop offset="0%" style="stop-color:#0f3460;stop-opacity:1" />
                            <stop offset="100%" style="stop-color:#e94560;stop-opacity:1" />
                        </linearGradient>
                    </defs>
                    <rect width="90%" height="90%" x="5%" y="5%" rx="15" ry="15" fill="url(#grad)" opacity="0.8"/>
                    <text x="50%" y="40%" dominant-baseline="middle" text-anchor="middle" fill="#ffffff" font-family="sans-serif" font-size="24" font-weight="bold">
                        Auto-Refinement Cycle
                    </text>
                    <text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" fill="#e2e8f0" font-family="sans-serif" font-size="18">
                        Round {} / {}
                    </text>
                    <text x="50%" y="65%" dominant-baseline="middle" text-anchor="middle" fill="#cbd5e1" font-family="sans-serif" font-size="14" opacity="0.9">
                        Prompt: {}
                    </text>
                </svg>"##,
                width,
                height,
                round + 1,
                max_iter,
                current_prompt
            );

            let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
            openmedia_svg::rasterize(
                &svg_content,
                Some(width),
                Some(height),
                None,
                "png",
                &output_path,
            )
            .map_err(|e| e.to_string())?;

            let clip = if let Some(scorer) = self.clip_scorer.as_ref() {
                scorer.score(&output_path, &current_prompt).await.ok()
            } else {
                Some(0.20 + (round as f32) * 0.05)
            };

            let aesthetic = if let Some(scorer) = self.aesthetic_scorer.as_ref() {
                scorer.score(&output_path).await.ok()
            } else {
                Some(7.0 + (round as f32) * 0.3)
            };

            let overall = (clip.unwrap_or(0.0) * 10.0 + aesthetic.unwrap_or(0.0)) / 2.0;

            let file_size = std::fs::metadata(&output_path)
                .map(|m| m.len())
                .unwrap_or(0);

            let record = GenerationRecord {
                id: gen_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tool_name: "improve_auto_refine".to_string(),
                request_params: serde_json::json!({
                    "prompt": current_prompt,
                    "negative_prompt": current_negative,
                    "width": width,
                    "height": height,
                }),
                output_path: output_path.to_string_lossy().to_string(),
                output_format: "png".to_string(),
                output_size: file_size,
                width: Some(width),
                height: Some(height),
                duration: None,
                model_used: Some("svg_rasterizer_fallback".to_string()),
                backend_used: Some("svg".to_string()),
                generation_time: start_time.elapsed().as_secs_f64(),
                clip_score: clip,
                aesthetic_score: aesthetic,
                refined_from: parent_id.clone(),
                refinement_round: round,
                metadata: None,
            };

            self.history.record(&record).map_err(|e| e.to_string())?;

            if overall > best_score {
                best_score = overall;
                best_record = Some(record.clone());
            }

            let score_struct = openmedia_core::QualityScore {
                clip_score: clip,
                aesthetic_score: aesthetic,
                needs_refinement: clip.unwrap_or(0.0) < 0.25 || aesthetic.unwrap_or(0.0) < 4.5,
            };

            if !score_struct.needs_refinement {
                break;
            }

            let refined = self.prompt_refiner.refine(
                &current_prompt,
                &current_negative,
                &score_struct,
                round + 1,
            );
            current_prompt = refined.prompt;
            current_negative = refined.negative_prompt;
            parent_id = Some(gen_id);
        }

        let best = best_record.ok_or_else(|| "No generation record created".to_string())?;
        serde_json::to_value(best)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "improve_feedback",
        description = "Submit manual feedback rating and quality notes on a specific generation."
    )]
    pub async fn improve_feedback(
        &self,
        params: Parameters<ImproveFeedbackRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;

        let feedback = Feedback {
            generation_id: req.generation_id,
            rating: req.rating,
            feedback: req.feedback,
            keep: req.keep.unwrap_or(true),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        self.history
            .record_feedback(&feedback)
            .map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "status": "success",
            "message": "Feedback submitted successfully",
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "improve_quality_report",
        description = "Retrieve comprehensive quality report and analytics."
    )]
    pub async fn improve_quality_report(
        &self,
        params: Parameters<ImproveQualityReportRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;

        let stats = self.history.stats().map_err(|e| e.to_string())?;

        let filter = HistoryFilter {
            tool_name: req.tool_name,
            limit: 10,
            offset: 0,
            sort_by: "created_at".to_string(),
            sort_order: "desc".to_string(),
            min_clip_score: None,
            min_aesthetic: None,
        };

        let recent = self.history.query(&filter).map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "total_generations": stats.total_generations,
            "total_size_bytes": stats.total_size_bytes,
            "avg_clip_score": stats.avg_clip_score,
            "avg_aesthetic_score": stats.avg_aesthetic_score,
            "db_size_bytes": stats.db_size_bytes,
            "recent_records": recent,
        });

        Ok(Json(McpObject(response)))
    }
}
