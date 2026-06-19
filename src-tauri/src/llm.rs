//! The LLM seam. The app always wires the real `OpenRouterLlm`; `FakeLlm` is test-only.
//! `cost_usd` is the actual cost reported by OpenRouter (read from the response), modeled as
//! `Option` so an unknown is never disguised as a real number.
// `OpenRouterLlm` (real) is added later in this task; the trait + types are consumed by
// prompts/chain. Suppress dead-code until wired.
#![allow(dead_code)]

pub struct LlmRequest {
    pub model: String,
    pub system: String,
    pub user: String,
}

pub struct LlmResponse {
    pub content: String,
    /// Actual cost from OpenRouter in **micro-dollars** (1_000_000 = $1.00), converted from
    /// `usage.cost` at the parse boundary. Integer so downstream sums stay exact; `None` if the
    /// response didn't report a cost — never fabricated.
    pub cost_micro_usd: Option<i64>,
}

pub trait Llm {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, String>;
}

const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Real LLM: OpenRouter's OpenAI-compatible chat-completions API. Auth is
/// `Authorization: Bearer <key>`. Actual cost is read from `usage.cost` in the response
/// (OpenRouter credits, USD-pegged 1:1; per their usage-accounting docs — now always
/// included). `cost_usd` is `None` if the field is absent, never fabricated.
///
/// **Runtime constraint:** like `ScrapingBeeScraper`, `reqwest::blocking` must run off the
/// tokio reactor (Tasks 4 & 6 execute steps on a dedicated worker thread / sync command).
pub struct OpenRouterLlm;

#[derive(serde::Deserialize)]
struct OrResponse {
    choices: Vec<OrChoice>,
    usage: Option<OrUsage>,
}
#[derive(serde::Deserialize)]
struct OrChoice {
    message: OrMessage,
}
#[derive(serde::Deserialize)]
struct OrMessage {
    content: Option<String>,
}
#[derive(serde::Deserialize)]
struct OrUsage {
    cost: Option<f64>,
}

impl Llm for OpenRouterLlm {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, String> {
        let key = crate::secrets::get_secret("openrouter_api_key")?;
        let body = serde_json::json!({
            "model": req.model,
            "messages": [
                {"role": "system", "content": req.system},
                {"role": "user", "content": req.user},
            ],
        });
        // LLM generation over a full careers page routinely exceeds the blocking client's 30s
        // DEFAULT timeout (that was the "Decode: TimedOut" failure); give it ample time.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("openrouter client build failed: {e}"))?;
        let resp = client
            .post(OPENROUTER_ENDPOINT)
            .bearer_auth(&key)
            .json(&body)
            .send()
            .map_err(|e| format!("openrouter request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .map_err(|e| format!("openrouter body read failed (HTTP {status}): {e:?}"))?;
        if !status.is_success() {
            return Err(format!("openrouter returned {status}: {text}"));
        }
        let parsed: OrResponse =
            serde_json::from_str(&text).map_err(|e| format!("openrouter response parse: {e}"))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or("openrouter: no message content in response")?;
        // Convert usage.cost (USD float) → micro-dollars once, here at the JSON boundary; None
        // if absent — recorded honestly, not fabricated.
        let cost_micro_usd = parsed
            .usage
            .and_then(|u| u.cost)
            .map(|c| (c * 1_000_000.0).round() as i64);
        Ok(LlmResponse { content, cost_micro_usd })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Test-only LLM. Reachable cross-module as `crate::llm::tests::FakeLlm`.
    pub struct FakeLlm {
        pub reply: String,
        pub cost_micro_usd: i64,
    }
    impl Llm for FakeLlm {
        fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse, String> {
            Ok(LlmResponse { content: self.reply.clone(), cost_micro_usd: Some(self.cost_micro_usd) })
        }
    }

    #[test]
    fn fake_echoes_reply_and_cost() {
        let l = FakeLlm { reply: "[]".into(), cost_micro_usd: 10_000 }; // $0.01
        let r = l
            .complete(&LlmRequest { model: "m".into(), system: "s".into(), user: "u".into() })
            .unwrap();
        assert_eq!(r.content, "[]");
        assert_eq!(r.cost_micro_usd, Some(10_000));
    }
}
