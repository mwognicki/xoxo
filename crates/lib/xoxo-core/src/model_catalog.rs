use llm_models_spider::model_profile;

/// Lookup summary for a model profile sourced from `llm_models_spider`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelLookupSummary {
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub input_cost_per_m_tokens: Option<f32>,
    pub output_cost_per_m_tokens: Option<f32>,
}

impl ModelLookupSummary {
    /// Estimate total USD cost from input and output token counts.
    pub fn estimate_total_cost_usd(
        &self,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Option<f32> {
        let input_cost = self.input_cost_per_m_tokens?;
        let output_cost = self.output_cost_per_m_tokens?;

        Some(
            (input_tokens as f32 / 1_000_000.0) * input_cost
                + (output_tokens as f32 / 1_000_000.0) * output_cost,
        )
    }

    /// Estimate remaining context percentage from total prompt-history tokens.
    pub fn context_left_percent(&self, used_tokens: u64) -> Option<u8> {
        if self.max_input_tokens == 0 {
            return None;
        }

        let max_input_tokens = self.max_input_tokens as u64;
        let remaining = max_input_tokens.saturating_sub(used_tokens);
        Some(((remaining * 100) / max_input_tokens) as u8)
    }
}

/// Lookup model metadata using exact and provider-stripped variants.
pub fn lookup_model_summary(model_name: &str) -> Option<ModelLookupSummary> {
    model_name_candidates(model_name).into_iter().find_map(|candidate| {
        model_profile(candidate).map(|profile| ModelLookupSummary {
            max_input_tokens: profile.max_input_tokens,
            max_output_tokens: profile.max_output_tokens,
            input_cost_per_m_tokens: profile.pricing.input_cost_per_m_tokens,
            output_cost_per_m_tokens: profile.pricing.output_cost_per_m_tokens,
        })
    })
}

fn model_name_candidates(model_name: &str) -> Vec<&str> {
    let mut candidates = vec![model_name];
    if let Some((_, stripped)) = model_name.rsplit_once('/') {
        candidates.push(stripped);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_supports_provider_prefixed_names() {
        let exact = lookup_model_summary("gpt-4o");
        let prefixed = lookup_model_summary("openai/gpt-4o");

        assert!(exact.is_some());
        assert_eq!(prefixed, exact);
    }

    #[test]
    fn context_left_percent_uses_max_input_tokens() {
        let summary = ModelLookupSummary {
            max_input_tokens: 1000,
            max_output_tokens: 100,
            input_cost_per_m_tokens: Some(1.0),
            output_cost_per_m_tokens: Some(2.0),
        };

        assert_eq!(summary.context_left_percent(250), Some(75));
    }

    #[test]
    fn estimate_total_cost_uses_input_and_output_rates() {
        let summary = ModelLookupSummary {
            max_input_tokens: 1000,
            max_output_tokens: 100,
            input_cost_per_m_tokens: Some(2.0),
            output_cost_per_m_tokens: Some(4.0),
        };

        let estimated = summary
            .estimate_total_cost_usd(500_000, 250_000)
            .expect("estimated cost");

        assert!((estimated - 2.0).abs() < f32::EPSILON);
    }
}
