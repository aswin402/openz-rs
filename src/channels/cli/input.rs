pub async fn read_line_raw(_prompt: &str, _history: &mut [String], _is_multiline: &mut bool) -> anyhow::Result<String> {
    Ok(String::new())
}
