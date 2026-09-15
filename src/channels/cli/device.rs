use crate::agent::style::*;
use anyhow::Result;

#[allow(unused_macros)]
macro_rules! println {
    () => {
        crate::tui_println!()
    };
    ($($arg:tt)*) => {
        crate::tui_println!($($arg)*)
    };
}

pub async fn handle_device_command(input: &str) -> Result<()> {
    use crate::tools::Tool;

    let tool = crate::tools::device_inventory::DeviceInventoryTool::new();
    let mut parts = input.split_whitespace();
    let action = parts.next().unwrap_or("help");

    let result = match action {
        "" | "help" => {
            print_device_usage();
            return Ok(());
        }
        "list" => {
            let category = parts.next();
            let mut args = serde_json::json!({"action": "list"});
            if let Some(category) = category {
                args["category"] = serde_json::json!(category);
            }
            tool.call(&args).await?
        }
        "suggest" => {
            let category = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");
            if category.is_empty() || target.is_empty() {
                println!("Usage: /device suggest <category> <target|extension>");
                return Ok(());
            }
            tool.call(&serde_json::json!({
                "action": "suggest",
                "category": category,
                "target": target,
            }))
            .await?
        }
        "add" => {
            let category = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            let command = parts.next().unwrap_or("");
            let works_for = parts.next().unwrap_or("");
            if category.is_empty() || name.is_empty() || command.is_empty() {
                println!("Usage: /device add <category> <name> <command> [works_for_csv]");
                return Ok(());
            }
            let works_for = split_csv(works_for);
            tool.call(&serde_json::json!({
                "action": "add",
                "category": category,
                "name": name,
                "command": command,
                "args": ["{target}"],
                "works_for": works_for,
                "source": "user",
            }))
            .await?
        }
        "delete" | "remove" => {
            let id = parts.next().unwrap_or("");
            if id.is_empty() {
                println!("Usage: /device delete <id>");
                return Ok(());
            }
            tool.call(&serde_json::json!({"action": "delete", "id": id}))
                .await?
        }
        "success" => {
            let id = parts.next().unwrap_or("");
            if id.is_empty() {
                println!("Usage: /device success <id>");
                return Ok(());
            }
            tool.call(&serde_json::json!({"action": "record_success", "id": id}))
                .await?
        }
        "failure" | "fail" => {
            let id = parts.next().unwrap_or("");
            if id.is_empty() {
                println!("Usage: /device failure <id>");
                return Ok(());
            }
            tool.call(&serde_json::json!({"action": "record_failure", "id": id}))
                .await?
        }
        other => {
            println!("Unknown /device action: {other}");
            print_device_usage();
            return Ok(());
        }
    };

    print_device_result(&result);
    Ok(())
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn print_device_usage() {
    println!("{}Device inventory commands:{}", COLOR_BOLD, COLOR_RESET);
    println!("  {} /device list [category]{}", RED_ORANGE, COLOR_RESET);
    println!(
        "  {} /device suggest <category> <target|extension>{}",
        RED_ORANGE, COLOR_RESET
    );
    println!(
        "  {} /device add <category> <name> <command> [works_for_csv]{}",
        RED_ORANGE, COLOR_RESET
    );
    println!("  {} /device delete <id>{}", RED_ORANGE, COLOR_RESET);
    println!("  {} /device success <id>{}", RED_ORANGE, COLOR_RESET);
    println!("  {} /device failure <id>{}", RED_ORANGE, COLOR_RESET);
}

pub(crate) fn print_device_result(result: &serde_json::Value) {
    if let Some(capabilities) = result
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
    {
        if capabilities.is_empty() {
            println!("No device capabilities saved.");
            return;
        }
        println!("{}Device capabilities:{}", COLOR_BOLD, COLOR_RESET);
        for item in capabilities {
            print_device_capability(item, None);
        }
        return;
    }

    if let Some(suggestions) = result
        .get("suggestions")
        .and_then(serde_json::Value::as_array)
    {
        if suggestions.is_empty() {
            println!("No matching device capability found.");
            return;
        }
        println!("{}Suggested capabilities:{}", COLOR_BOLD, COLOR_RESET);
        for item in suggestions {
            let score = item.get("score").and_then(serde_json::Value::as_f64);
            if let Some(capability) = item.get("capability") {
                print_device_capability(capability, score);
            }
        }
        return;
    }

    if let Some(capability) = result.get("capability") {
        print_device_capability(capability, None);
        return;
    }

    if let Some(deleted) = result.get("deleted").and_then(serde_json::Value::as_str) {
        println!(
            "{}✓ Deleted device capability {}.{}",
            EMERALD_GREEN, deleted, COLOR_RESET
        );
        return;
    }

    println!("{}", result);
}

pub(crate) fn print_device_capability(item: &serde_json::Value, score: Option<f64>) {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<unknown>");
    let category = item
        .get("category")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unnamed");
    let command = item
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let confidence = item
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let success = item
        .get("success_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let failure = item
        .get("failure_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let works_for = item
        .get("works_for")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "any".to_string());
    let score_suffix = score
        .map(|score| format!(" score={score:.1}"))
        .unwrap_or_default();

    println!(
        "  {}{}{} [{}] {} cmd={} works_for={} ok={} fail={} conf={:.2}{}",
        RED_ORANGE,
        id,
        COLOR_RESET,
        category,
        name,
        command,
        works_for,
        success,
        failure,
        confidence,
        score_suffix
    );
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
