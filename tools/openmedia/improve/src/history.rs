use std::sync::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use openmedia_core::{Result, OpenMediaError};
use crate::{GenerationRecord, HistoryFilter, HistoryStats, Feedback};

pub struct GenerationHistory {
    conn: Mutex<Connection>,
}

impl GenerationHistory {
    pub fn open(db_path: &std::path::Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path)
            .map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        conn.execute_batch(include_str!("../sql/schema.sql"))
            .map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn record(&self, entry: &GenerationRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let request_params_str = entry.request_params.to_string();
        let metadata_str = entry.metadata.as_ref().map(|m| m.to_string());

        conn.execute(
            "INSERT OR REPLACE INTO generations (
                id, created_at, tool_name, request_params, output_path, output_format,
                output_size, width, height, duration, model_used, backend_used,
                generation_time, clip_score, aesthetic_score, refined_from,
                refinement_round, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                entry.id,
                entry.created_at,
                entry.tool_name,
                request_params_str,
                entry.output_path,
                entry.output_format,
                entry.output_size as i64,
                entry.width,
                entry.height,
                entry.duration,
                entry.model_used,
                entry.backend_used,
                entry.generation_time,
                entry.clip_score,
                entry.aesthetic_score,
                entry.refined_from,
                entry.refinement_round,
                metadata_str
            ],
        )
        .map_err(|e| OpenMediaError::DatabaseError(format!("Failed to insert record: {}", e)))?;

        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<GenerationRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, tool_name, request_params, output_path, output_format,
                    output_size, width, height, duration, model_used, backend_used,
                    generation_time, clip_score, aesthetic_score, refined_from,
                    refinement_round, metadata
             FROM generations WHERE id = ?"
        ).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        let record = stmt.query_row(params![id], |row| {
            let request_params_str: String = row.get(3)?;
            let metadata_str: Option<String> = row.get(17)?;

            let request_params = serde_json::from_str(&request_params_str).unwrap_or(serde_json::Value::Null);
            let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

            let output_size_i64: i64 = row.get(6)?;

            Ok(GenerationRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                tool_name: row.get(2)?,
                request_params,
                output_path: row.get(4)?,
                output_format: row.get(5)?,
                output_size: output_size_i64 as u64,
                width: row.get(7)?,
                height: row.get(8)?,
                duration: row.get(9)?,
                model_used: row.get(10)?,
                backend_used: row.get(11)?,
                generation_time: row.get(12)?,
                clip_score: row.get(13)?,
                aesthetic_score: row.get(14)?,
                refined_from: row.get(15)?,
                refinement_round: row.get(16)?,
                metadata,
            })
        }).optional().map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        Ok(record)
    }

    pub fn query(&self, filter: &HistoryFilter) -> Result<Vec<GenerationRecord>> {
        let conn = self.conn.lock().unwrap();
        
        let mut query_str = String::from(
            "SELECT id, created_at, tool_name, request_params, output_path, output_format,
                    output_size, width, height, duration, model_used, backend_used,
                    generation_time, clip_score, aesthetic_score, refined_from,
                    refinement_round, metadata
             FROM generations WHERE 1=1"
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(tool) = &filter.tool_name {
            query_str.push_str(" AND tool_name = ?");
            params_vec.push(Box::new(tool.clone()));
        }

        if let Some(min_clip) = filter.min_clip_score {
            query_str.push_str(" AND clip_score >= ?");
            params_vec.push(Box::new(min_clip));
        }

        if let Some(min_aes) = filter.min_aesthetic {
            query_str.push_str(" AND aesthetic_score >= ?");
            params_vec.push(Box::new(min_aes));
        }

        // Validate sorting fields to prevent SQL injection
        let order_by = match filter.sort_by.as_str() {
            "created_at" => "created_at",
            "clip_score" => "clip_score",
            "aesthetic_score" => "aesthetic_score",
            "generation_time" => "generation_time",
            _ => "created_at",
        };

        let order_dir = match filter.sort_order.to_lowercase().as_str() {
            "asc" => "ASC",
            _ => "DESC",
        };

        query_str.push_str(&format!(" ORDER BY {} {}", order_by, order_dir));
        query_str.push_str(" LIMIT ? OFFSET ?");
        params_vec.push(Box::new(filter.limit as i64));
        params_vec.push(Box::new(filter.offset as i64));

        let mut stmt = conn.prepare(&query_str).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        // Borrow elements as &dyn ToSql
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

        let rows = stmt.query_map(&params_refs[..], |row| {
            let request_params_str: String = row.get(3)?;
            let metadata_str: Option<String> = row.get(17)?;

            let request_params = serde_json::from_str(&request_params_str).unwrap_or(serde_json::Value::Null);
            let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

            let output_size_i64: i64 = row.get(6)?;

            Ok(GenerationRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                tool_name: row.get(2)?,
                request_params,
                output_path: row.get(4)?,
                output_format: row.get(5)?,
                output_size: output_size_i64 as u64,
                width: row.get(7)?,
                height: row.get(8)?,
                duration: row.get(9)?,
                model_used: row.get(10)?,
                backend_used: row.get(11)?,
                generation_time: row.get(12)?,
                clip_score: row.get(13)?,
                aesthetic_score: row.get(14)?,
                refined_from: row.get(15)?,
                refinement_round: row.get(16)?,
                metadata,
            })
        }).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?);
        }

        Ok(results)
    }

    pub fn record_feedback(&self, feedback: &Feedback) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO feedback (generation_id, rating, feedback_text, keep_output, created_at)
             VALUES (?, ?, ?, ?, ?)",
            params![
                feedback.generation_id,
                feedback.rating,
                feedback.feedback,
                feedback.keep as i32,
                feedback.created_at
            ],
        )
        .map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn stats(&self) -> Result<HistoryStats> {
        let conn = self.conn.lock().unwrap();

        let total_generations: u64 = conn.query_row(
            "SELECT COUNT(*) FROM generations",
            [],
            |row| row.get(0),
        ).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        let total_size_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(output_size), 0) FROM generations",
            [],
            |row| row.get(0),
        ).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        let avg_clip_score: Option<f32> = conn.query_row(
            "SELECT AVG(clip_score) FROM generations",
            [],
            |row| row.get(0),
        ).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        let avg_aesthetic_score: Option<f32> = conn.query_row(
            "SELECT AVG(aesthetic_score) FROM generations",
            [],
            |row| row.get(0),
        ).map_err(|e| OpenMediaError::DatabaseError(e.to_string()))?;

        // Simple approximate DB file size from metadata or PRAGMA page_count * page_size
        let db_size_bytes: i64 = conn.query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(HistoryStats {
            total_generations,
            total_size_bytes: total_size_bytes as u64,
            avg_clip_score,
            avg_aesthetic_score,
            db_size_bytes: db_size_bytes as u64,
        })
    }
}
