use xoxo_core::model_catalog::lookup_model_summary;

pub(super) fn derive_model_stats(
    current_model_name: &str,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_used_tokens: u64,
) -> (Option<u8>, Option<u32>, Option<f32>) {
    let model_summary = lookup_model_summary(current_model_name);
    let context_left_percent = model_summary
        .as_ref()
        .and_then(|summary| summary.context_left_percent(total_used_tokens));
    let max_input_tokens = model_summary.as_ref().map(|summary| summary.max_input_tokens);
    let estimated_cost_usd = model_summary.as_ref().and_then(|summary| {
        summary.estimate_total_cost_usd(total_input_tokens, total_output_tokens)
    });

    (context_left_percent, max_input_tokens, estimated_cost_usd)
}
