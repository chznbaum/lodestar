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
    /// Optional **cached leading user prefix** — a stable RAG-style reference block that precedes
    /// the volatile per-call `user` content. When `Some`, `build_or_body` emits the user message
    /// as a multipart array `[{prefix, cache_control: ephemeral 1h}, {user}]`: the prefix carries
    /// the single cache breakpoint (caching `system` + prefix), the trailing per-call content does
    /// not. When `None`, the user message is emitted as a plain string exactly as before. The
    /// breakpoint is added unconditionally when `Some` — it is harmless and ignored on
    /// non-Anthropic providers, so no model gating is needed. Only `build_alignment_prompt` sets
    /// this (the candidate dossier); every other builder leaves it `None`.
    pub cached_prefix: Option<String>,
}

pub struct LlmResponse {
    pub content: String,
    /// Actual cost from OpenRouter in **micro-dollars** (1_000_000 = $1.00), converted from
    /// `usage.cost` at the parse boundary. Integer so downstream sums stay exact; `None` if the
    /// response didn't report a cost — never fabricated.
    pub cost_micro_usd: Option<i64>,
    /// Cache **read** tokens for this call — from `usage.prompt_tokens_details.cached_tokens`.
    /// `> 0` proves the cached prefix was served from cache (the cardinal "is caching engaging?"
    /// signal). `None` when the provider didn't report it — never fabricated. `usage.cost` already
    /// nets the cache discount, so this is visibility, not accounting.
    pub cache_read_tokens: Option<i64>,
    /// Cache **write** tokens for this call — from `usage.prompt_tokens_details.cache_write_tokens`.
    /// `> 0` on the first call / each 1h re-warm (the prefix being written to cache). `None` when
    /// the provider didn't report it — never fabricated.
    pub cache_write_tokens: Option<i64>,
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
    /// Top-level cost delta from caching (negative on writes, positive on reads for Anthropic).
    /// Captured for completeness; `cost` already nets it, so this is informational only.
    #[serde(default)]
    cache_discount: Option<f64>,
    /// Cache-token detail nesting. Absent on providers/calls that don't report caching — then the
    /// whole struct deserializes to `None` and the cache token fields surface as `None` (never
    /// fabricated).
    #[serde(default)]
    prompt_tokens_details: Option<OrPromptTokensDetails>,
}
#[derive(serde::Deserialize)]
struct OrPromptTokensDetails {
    /// Cache **read** tokens (prefix served from cache).
    #[serde(default)]
    cached_tokens: Option<i64>,
    /// Cache **write** tokens (prefix written to cache on first call / re-warm).
    #[serde(default)]
    cache_write_tokens: Option<i64>,
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
    // The user content is either a plain string (no caching — byte-for-byte the prior behavior) or
    // a multipart array when a `cached_prefix` is set: part 0 is the cached dossier carrying the
    // ephemeral 1h cache_control breakpoint, the trailing part is the volatile per-call content
    // with NO breakpoint (caching is a strict prefix match, so the breakpoint must end the cached
    // span). The breakpoint is added unconditionally when `cached_prefix` is `Some` — harmless and
    // ignored on non-Anthropic providers.
    let user_content = match &req.cached_prefix {
        Some(prefix) => serde_json::json!([
            {"type": "text", "text": prefix, "cache_control": {"type": "ephemeral", "ttl": "1h"}},
            {"type": "text", "text": req.user},
        ]),
        None => serde_json::Value::String(req.user.clone()),
    };
    let messages = serde_json::json!([
        {"role": "system", "content": req.system},
        {"role": "user", "content": user_content},
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
        parse_or_response(&text)
    }
}

/// Parse an OpenRouter chat-completions response body into an `LlmResponse`. Extracted as a pure
/// function (no network) so the cost + cache-telemetry capture is unit-testable. Every usage field
/// is `Option` and surfaces `None` when absent — never fabricated (project posture).
fn parse_or_response(text: &str) -> Result<LlmResponse, String> {
    let parsed: OrResponse =
        serde_json::from_str(text).map_err(|e| format!("openrouter response parse: {e}"))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or("openrouter: no message content in response")?;
    // Convert usage.cost (USD float) → micro-dollars once, here at the JSON boundary; None if
    // absent — recorded honestly, not fabricated. Cache read/write tokens come from the nested
    // prompt_tokens_details; each is None when the provider didn't report it.
    let usage = parsed.usage;
    let cost_micro_usd = usage
        .as_ref()
        .and_then(|u| u.cost)
        .map(|c| (c * 1_000_000.0).round() as i64);
    let details = usage.as_ref().and_then(|u| u.prompt_tokens_details.as_ref());
    let cache_read_tokens = details.and_then(|d| d.cached_tokens);
    let cache_write_tokens = details.and_then(|d| d.cache_write_tokens);
    Ok(LlmResponse { content, cost_micro_usd, cache_read_tokens, cache_write_tokens })
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
            // `web` and `cached_prefix` are intentionally ignored — FakeLlm is zero-spend,
            // network-free, and reports no cache telemetry.
            Ok(LlmResponse {
                content: self.reply.clone(),
                cost_micro_usd: Some(self.cost_micro_usd),
                cache_read_tokens: None,
                cache_write_tokens: None,
            })
        }
    }

    #[test]
    fn fake_echoes_reply_and_cost() {
        let l = FakeLlm { reply: "[]".into(), cost_micro_usd: 10_000 }; // $0.01
        let r = l
            .complete(&LlmRequest { model: "m".into(), system: "s".into(), user: "u".into(), web: false, cached_prefix: None })
            .unwrap();
        assert_eq!(r.content, "[]");
        assert_eq!(r.cost_micro_usd, Some(10_000));
    }

    #[test]
    fn fake_works_with_web_true() {
        // FakeLlm must ignore web entirely — same reply regardless.
        let l = FakeLlm { reply: "ok".into(), cost_micro_usd: 0 };
        let r = l
            .complete(&LlmRequest { model: "m".into(), system: "s".into(), user: "u".into(), web: true, cached_prefix: None })
            .unwrap();
        assert_eq!(r.content, "ok");
    }

    // ── build_or_body unit tests (pure, no network) ───────────────────────

    #[test]
    fn build_or_body_no_web_has_no_tools_key() {
        let req = LlmRequest { model: "some/model".into(), system: "sys".into(), user: "usr".into(), web: false, cached_prefix: None };
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
        let req = LlmRequest { model: "some/model".into(), system: "sys".into(), user: "usr".into(), web: true, cached_prefix: None };
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
        let req = LlmRequest { model: "m".into(), system: "SYSTEM_CONTENT".into(), user: "USER_CONTENT".into(), web: false, cached_prefix: None };
        let body = build_or_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "SYSTEM_CONTENT");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "USER_CONTENT");
    }

    // ── cached_prefix (prompt-caching breakpoint) unit tests (pure, no network) ─

    #[test]
    fn build_or_body_cached_prefix_some_emits_multipart_with_breakpoint() {
        // When cached_prefix is Some, the user content becomes a multipart array: part 0 is the
        // cached prefix carrying the ephemeral 1h cache_control breakpoint; the last part is the
        // per-job content and carries NO breakpoint (caching is a strict prefix match).
        let req = LlmRequest {
            model: "anthropic/claude-opus-4.8".into(),
            system: "SYS".into(),
            user: "PER_JOB_SUFFIX".into(),
            web: false,
            cached_prefix: Some("CACHED_DOSSIER".into()),
        };
        let body = build_or_body(&req);

        // Model + messages intact.
        assert_eq!(body["model"], "anthropic/claude-opus-4.8");
        let msgs = body["messages"].as_array().expect("messages array");
        assert_eq!(msgs.len(), 2);
        // System message still a plain string, present.
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "SYS");

        // User content is a multipart array.
        assert_eq!(msgs[1]["role"], "user");
        let parts = msgs[1]["content"].as_array().expect("user content must be a multipart array");
        assert_eq!(parts.len(), 2, "exactly two parts: cached prefix + per-job suffix");

        // Part 0: the cached prefix, with the ephemeral 1h breakpoint.
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "CACHED_DOSSIER");
        assert_eq!(
            parts[0]["cache_control"],
            serde_json::json!({"type": "ephemeral", "ttl": "1h"}),
            "part 0 must carry the ephemeral 1h cache_control breakpoint"
        );

        // Last part: the per-job content, NO cache_control.
        assert_eq!(parts[1]["type"], "text");
        assert_eq!(parts[1]["text"], "PER_JOB_SUFFIX");
        assert!(
            parts[1].get("cache_control").is_none(),
            "the trailing per-job part must NOT carry a cache_control breakpoint"
        );
    }

    #[test]
    fn build_or_body_cached_prefix_none_emits_plain_string_no_cache_control() {
        // Regression guard for the other 3 builders: with cached_prefix None the user content is a
        // plain JSON string exactly as before, and no cache_control key appears anywhere.
        let req = LlmRequest { model: "m".into(), system: "SYS".into(), user: "USR".into(), web: false, cached_prefix: None };
        let body = build_or_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        // User content is a plain string (current behavior), not an array.
        assert!(msgs[1]["content"].is_string(), "user content must stay a plain string when cached_prefix is None");
        assert_eq!(msgs[1]["content"], "USR");
        // No cache_control anywhere in the serialized body.
        let serialized = serde_json::to_string(&body).unwrap();
        assert!(!serialized.contains("cache_control"), "no cache_control key may appear when cached_prefix is None");
    }

    #[test]
    fn build_or_body_web_true_works_with_cached_prefix() {
        // The web tools array is independent of the user-content shape: web:true + a cached prefix
        // must still produce the tools array AND the multipart cached user content.
        let req = LlmRequest {
            model: "m".into(),
            system: "SYS".into(),
            user: "USR".into(),
            web: true,
            cached_prefix: Some("PREFIX".into()),
        };
        let body = build_or_body(&req);
        // Tools array unaffected.
        let tools = body["tools"].as_array().expect("web:true body must have a tools array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "openrouter:web_search");
        assert_eq!(body["tool_choice"], "auto");
        // User content is the multipart cached array.
        let msgs = body["messages"].as_array().unwrap();
        let parts = msgs[1]["content"].as_array().expect("user content must be multipart with cached_prefix");
        assert_eq!(parts[0]["text"], "PREFIX");
        assert_eq!(parts[0]["cache_control"], serde_json::json!({"type": "ephemeral", "ttl": "1h"}));
    }

    // ── usage cache-telemetry parse tests ─────────────────────────────────────

    #[test]
    fn parse_usage_captures_cache_tokens_when_present() {
        // An OpenRouter response carrying prompt_tokens_details.{cached_tokens,cache_write_tokens}
        // and top-level usage.cache_discount must parse those onto the LlmResponse honestly.
        let text = r#"{
            "choices": [{"message": {"content": "ok"}}],
            "usage": {
                "cost": 0.5,
                "cache_discount": -0.012,
                "prompt_tokens_details": {"cached_tokens": 4096, "cache_write_tokens": 7000}
            }
        }"#;
        let resp = parse_or_response(text).unwrap();
        assert_eq!(resp.content, "ok");
        assert_eq!(resp.cost_micro_usd, Some(500_000)); // $0.50
        assert_eq!(resp.cache_read_tokens, Some(4096));
        assert_eq!(resp.cache_write_tokens, Some(7000));
    }

    #[test]
    fn parse_usage_cache_tokens_none_when_absent() {
        // A response without prompt_tokens_details must leave the cache token fields None —
        // never fabricated (project posture).
        let text = r#"{
            "choices": [{"message": {"content": "ok"}}],
            "usage": {"cost": 0.25}
        }"#;
        let resp = parse_or_response(text).unwrap();
        assert_eq!(resp.cost_micro_usd, Some(250_000));
        assert_eq!(resp.cache_read_tokens, None);
        assert_eq!(resp.cache_write_tokens, None);
    }
}
