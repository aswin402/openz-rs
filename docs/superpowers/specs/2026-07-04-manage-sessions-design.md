# Design Spec — Session Management & Storage Curation (`manage_sessions` tool)

This spec details the architectural design and parameter specifications for the `manage_sessions` tool.

---

## 🎯 Objectives
Provide the OpenZ agent with an in-process session management tool to:
1. List all active sessions with metadata (disk size, message count, last updated).
2. Prune old temporary tool output JSON files to reclaim storage space.
3. Archive or permanently delete session histories when they are no longer needed.

---

## ⚙️ JSON Schema Specifications

### `manage_sessions` Parameter Schema
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["list", "prune", "archive", "delete"],
      "description": "The curation action to perform."
    },
    "session_key": {
      "type": "string",
      "description": "Required for 'archive' or 'delete'. The key of the session to target."
    },
    "older_than_days": {
      "type": "integer",
      "description": "Optional for 'prune'. Delete tool output files older than this number of days. Default is 7."
    }
  },
  "required": ["action"]
}
```

### Result Schema
* **For `list`**:
```json
{
  "status": "success",
  "sessions": [
    {
      "session_key": "session_name",
      "size_bytes": 1024,
      "message_count": 15,
      "last_updated": "2026-07-04T12:00:00Z"
    }
  ]
}
```
* **For `prune` / `archive` / `delete`**:
```json
{
  "status": "success",
  "message": "Curation operation completed successfully.",
  "details": {
    "files_removed": 14,
    "bytes_reclaimed": 54120
  }
}
```
