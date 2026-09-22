-- OpenMedia-RS Generation History Schema
-- Database: ~/.openmedia/history.db

-- Main generations table
CREATE TABLE IF NOT EXISTS generations (
    id              TEXT PRIMARY KEY,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    tool_name       TEXT NOT NULL,
    request_params  TEXT NOT NULL,          -- JSON
    output_path     TEXT NOT NULL,
    output_format   TEXT NOT NULL,
    output_size     INTEGER NOT NULL,       -- bytes
    width           INTEGER,
    height          INTEGER,
    duration        REAL,                   -- seconds (video/animation)
    model_used      TEXT,
    backend_used    TEXT,
    generation_time REAL NOT NULL,          -- wall-clock seconds
    clip_score      REAL,
    aesthetic_score REAL,
    refined_from    TEXT REFERENCES generations(id),
    refinement_round INTEGER NOT NULL DEFAULT 0,
    metadata        TEXT                    -- additional JSON
);

-- Feedback table
CREATE TABLE IF NOT EXISTS feedback (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    generation_id   TEXT NOT NULL REFERENCES generations(id) ON DELETE CASCADE,
    rating          REAL NOT NULL CHECK (rating >= 0.0 AND rating <= 1.0),
    feedback_text   TEXT,
    keep_output     INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_gen_tool ON generations(tool_name);
CREATE INDEX IF NOT EXISTS idx_gen_created ON generations(created_at);
CREATE INDEX IF NOT EXISTS idx_gen_model ON generations(model_used);
CREATE INDEX IF NOT EXISTS idx_gen_refined ON generations(refined_from);
CREATE INDEX IF NOT EXISTS idx_gen_clip ON generations(clip_score);
CREATE INDEX IF NOT EXISTS idx_gen_aesthetic ON generations(aesthetic_score);
CREATE INDEX IF NOT EXISTS idx_feedback_gen ON feedback(generation_id);

-- View for generation statistics
CREATE VIEW IF NOT EXISTS generation_stats AS
SELECT
    tool_name,
    COUNT(*) as total_count,
    AVG(generation_time) as avg_time,
    AVG(clip_score) as avg_clip,
    AVG(aesthetic_score) as avg_aesthetic,
    SUM(output_size) as total_size,
    MIN(created_at) as first_gen,
    MAX(created_at) as last_gen
FROM generations
GROUP BY tool_name;
