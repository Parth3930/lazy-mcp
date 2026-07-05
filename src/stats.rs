use rmcp::model::Tool;

// ponytail: minimum that works
pub fn count_tokens(tool: &Tool) -> usize {
    let json = serde_json::to_string(&tool).unwrap_or_default();
    tiktoken_rs::cl100k_base().unwrap().encode_with_special_tokens(&json).len()
}
