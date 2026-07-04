# Design Spec — Configuration & Skills Snapshot Management (`manage_backups` tool)

This spec details the architectural design and parameter specifications for the `manage_backups` tool.

---

## 🎯 Objectives
Provide the OpenZ agent with an in-process backup manager to:
1. Snapshot active configurations (`config.json`), custom subagent profiles (`subagents.json`), and database/markdown skills.
2. List available snapshots with file sizes and creation times.
3. Restore system state safely from any selected snapshot.

---

## ⚙️ JSON Schema Specifications

### `manage_backups` Parameter Schema
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["create", "list", "restore", "delete"],
      "description": "The backup curation action to perform."
    },
    "backup_name": {
      "type": "string",
      "description": "Required for 'restore' or 'delete'. The filename of the backup target."
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
  "backups": [
    {
      "backup_name": "backup_20260704_130000.json",
      "size_bytes": 2048,
      "created_at": "2026-07-04T13:00:00Z"
    }
  ]
}
```
* **For `create` / `restore` / `delete`**:
```json
{
  "status": "success",
  "message": "Backup operation completed successfully."
}
```
