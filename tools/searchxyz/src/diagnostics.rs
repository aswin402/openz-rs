use crate::crawler::FetchCacheMode;
use crate::pipeline::SearchAndReadReport;
use crate::search::SearchReport;

pub(crate) fn format_search_diagnostics(report: &SearchReport) -> String {
    let mut out = format!(
        "\n---\n## Search Diagnostics\n- **Mode:** {}\n",
        report.mode
    );
    for attempt in &report.attempts {
        out.push_str(&format!(
            "- **{}:** {} (usable_results={})",
            attempt.backend, attempt.status, attempt.usable_results
        ));
        if let Some(detail) = &attempt.detail {
            out.push_str(&format!(" -- {}", compact_detail(detail)));
        }
        out.push('\n');
    }
    out
}

pub(crate) fn format_read_url_diagnostics(
    url: &str,
    cache_mode: FetchCacheMode,
    save: bool,
    path: &str,
) -> String {
    format!(
        "\n---\n## Fetch Diagnostics\n- **URL:** {}\n- **Path:** {}\n- **Cache Mode:** {:?}\n- **Saved:** {}\n",
        url, path, cache_mode, save
    )
}

pub(crate) fn format_search_and_read_diagnostics(
    report: &SearchAndReadReport,
    cache_mode: FetchCacheMode,
    save: bool,
) -> String {
    let mut out = format_search_diagnostics(&report.search);
    out.push_str(&format!(
        "\n## Page Diagnostics\n- **Cache Mode:** {:?}\n- **Saved:** {}\n",
        cache_mode, save
    ));
    for attempt in &report.page_attempts {
        out.push_str(&format!("- **{}:** {}", attempt.url, attempt.status));
        if let Some(detail) = &attempt.detail {
            out.push_str(&format!(" -- {}", compact_detail(detail)));
        }
        out.push('\n');
    }
    out
}

fn compact_detail(detail: &str) -> String {
    let compact = detail.replace('\n', " ");
    compact.chars().take(300).collect()
}
