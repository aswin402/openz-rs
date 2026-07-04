# Design Spec — System Health Diagnostics (`diagnose_system` tool)

This spec details the architectural design and parameter specifications for the `diagnose_system` tool.

---

## 🎯 Objectives
Provide the OpenZ agent with an in-process diagnostic tool to analyze:
1. Disk utilization of OpenZ storage directories to warn of/prevent disk exhaustion.
2. Health and size of the 5 internal SQLite databases used for cognitive memories, caching, docsets, and reasoning.
3. Real-time request latencies to active LLM providers.
4. General system load information.

---

## ⚙️ JSON Schema Specifications

### `diagnose_system` Parameter Schema
```json
{
  "type": "object",
  "properties": {
    "check_latency": {
      "type": "boolean",
      "description": "If true, tests HTTP ping round-trip times to active provider endpoints. Default is true."
    },
    "check_db_integrity": {
      "type": "boolean",
      "description": "If true, executes 'PRAGMA integrity_check;' on SQLite files. Can take longer. Default is false."
    }
  }
}
```

### Result Schema
```json
{
  "type": "object",
  "properties": {
    "status": { "type": "string" },
    "system": {
      "type": "object",
      "properties": {
        "os": { "type": "string" },
        "architecture": { "type": "string" },
        "cores": { "type": "integer" }
      }
    },
    "directories": {
      "type": "object",
      "patternProperties": {
        ".*": {
          "type": "object",
          "properties": {
            "path": { "type": "string" },
            "size_bytes": { "type": "integer" },
            "file_count": { "type": "integer" }
          }
        }
      }
    },
    "databases": {
      "type": "object",
      "patternProperties": {
        ".*": {
          "type": "object",
          "properties": {
            "path": { "type": "string" },
            "size_bytes": { "type": "integer" },
            "exists": { "type": "boolean" },
            "connectable": { "type": "boolean" },
            "integrity": { "type": "string" }
          }
        }
      }
    },
    "network": {
      "type": "object",
      "patternProperties": {
        ".*": {
          "type": "object",
          "properties": {
            "endpoint": { "type": "string" },
            "latency_ms": { "type": "integer" },
            "status": { "type": "string" }
          }
        }
      }
    }
  }
}
```
