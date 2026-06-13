---
name: social_search
description: "AI Agent Skill for Social Media Search — outlines how to query Twitter (Nitter), Reddit (public json API), and YouTube (Invidious) for free."
metadata:
  version: 1.0.0
---

# Social Media Search Skill 🌐🔍

This skill outlines how to query public social platforms without developer accounts or API keys using OpenZ.

## 1. Tool Usage

Invoke the `social_search` tool to execute a query:

```json
{
  "platform": "reddit",
  "query": "Rust compiler error E0502"
}
```

### Supported Platforms:
* **`reddit`**: Queries `old.reddit.com/search.json` directly to retrieve titles, post authors, and snippets.
* **`twitter`**: Scrapes public Nitter timeline instances to retrieve matching username handles and tweet texts.
* **`youtube`**: Queries public Invidious search APIs, falling back to raw HTML scraping to retrieve video URLs and channel descriptions.

---

## 2. Guidelines for Search
* Use this tool when you need to research developer discussions, community solutions, video walk-throughs, or real-time event feeds.
* Prefer `social_search` over generic `web_search` when looking for forum answers (like Reddit post threads) or quick social updates.
