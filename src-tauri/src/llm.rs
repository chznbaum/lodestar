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
    /// When `true`, the request uses OpenRouter's `openrouter:web_search` server tool so the
    /// model can perform live web searches during generation. Only `research-gaps` sets this.
    /// `FakeLlm` ignores this field entirely (stays zero-spend, network-free).
    pub web: bool,
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

/// Build the JSON request body for an OpenRouter chat-completions call.
///
/// When `req.web == true` the body includes the `openrouter:web_search` server tool
/// (OpenRouter's current recommended mechanism — the older `plugins` approach and `:online`
/// suffix are both deprecated as of 2026). When `false` the body is the standard
/// two-message completions request with no tool additions.
///
/// This is extracted as a pure function so it can be unit-tested without any network call.
pub fn build_or_body(req: &LlmRequest) -> serde_json::Value {
    let messages = serde_json::json!([
        {"role": "system", "content": req.system},
        {"role": "user", "content": req.user},
    ]);

    if req.web {
        serde_json::json!({
            "model": req.model,
            "messages": messages,
            "tools": [{"type": "openrouter:web_search"}],
            "tool_choice": "auto",
        })
    } else {
        serde_json::json!({
            "model": req.model,
            "messages": messages,
        })
    }
}

impl Llm for OpenRouterLlm {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, String> {
        let key = crate::secrets::get_secret("openrouter_api_key")?;
        let body = build_or_body(req);
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
            // `web` is intentionally ignored — FakeLlm is zero-spend and network-free.
            Ok(LlmResponse { content: self.reply.clone(), cost_micro_usd: Some(self.cost_micro_usd) })
        }
    }

    #[test]
    fn fake_echoes_reply_and_cost() {
        let l = FakeLlm { reply: "[]".into(), cost_micro_usd: 10_000 }; // $0.01
        let r = l
            .complete(&LlmRequest { model: "m".into(), system: "s".into(), user: "u".into(), web: false })
            .unwrap();
        assert_eq!(r.content, "[]");
        assert_eq!(r.cost_micro_usd, Some(10_000));
    }

    #[test]
    fn fake_works_with_web_true() {
        // FakeLlm must ignore web entirely — same reply regardless.
        let l = FakeLlm { reply: "ok".into(), cost_micro_usd: 0 };
        let r = l
            .complete(&LlmRequest { model: "m".into(), system: "s".into(), user: "u".into(), web: true })
            .unwrap();
        assert_eq!(r.content, "ok");
    }

    // ── build_or_body unit tests (pure, no network) ───────────────────────

    #[test]
    fn build_or_body_no_web_has_no_tools_key() {
        let req = LlmRequest { model: "some/model".into(), system: "sys".into(), user: "usr".into(), web: false };
        let body = build_or_body(&req);
        assert_eq!(body["model"], "some/model");
        // Non-web body must NOT contain a tools key.
        assert!(body.get("tools").is_none(), "web:false body must not include a tools key");
        // Must not contain tool_choice either (only meaningful when tools are present).
        assert!(body.get("tool_choice").is_none(), "web:false body must not include a tool_choice key");
        // Must not contain plugins key either (deprecated path, never used).
        assert!(body.get("plugins").is_none(), "web:false body must not include a plugins key");
        // Messages must be present.
        assert!(body["messages"].is_array());
    }

    #[test]
    fn build_or_body_web_true_includes_openrouter_web_search_tool() {
        let req = LlmRequest { model: "some/model".into(), system: "sys".into(), user: "usr".into(), web: true };
        let body = build_or_body(&req);
        // Must include a tools array.
        let tools = body["tools"].as_array().expect("web:true body must have a tools array");
        assert_eq!(tools.len(), 1, "exactly one tool entry");
        assert_eq!(
            tools[0]["type"].as_str().unwrap(),
            "openrouter:web_search",
            "tool type must be openrouter:web_search"
        );
        // Must include tool_choice: "auto" — makes the default explicit and stable against
        // any future OpenRouter default change.
        assert_eq!(
            body["tool_choice"].as_str().unwrap(),
            "auto",
            "web:true body must set tool_choice to auto"
        );
        // Must NOT include legacy plugins key.
        assert!(body.get("plugins").is_none(), "web:true body must not use deprecated plugins key");
        // Model and messages still present.
        assert_eq!(body["model"], "some/model");
        assert!(body["messages"].is_array());
    }

    #[test]
    fn build_or_body_messages_embed_system_and_user() {
        let req = LlmRequest { model: "m".into(), system: "SYSTEM_CONTENT".into(), user: "USER_CONTENT".into(), web: false };
        let body = build_or_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "SYSTEM_CONTENT");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "USER_CONTENT");
    }
}
