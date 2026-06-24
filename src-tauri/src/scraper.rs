//! The scraping seam. The app always wires the real `ScrapingBeeScraper`; `FakeScraper`
//! is test-only. `fetch` returns the page content + the ScrapingBee credits consumed.
// Trait + result type are public API consumed by the pipeline (Tasks 4–6); the real
// `ScrapingBeeScraper` lands with Task 6 (ScrapingBee Request Builder). Suppress dead-code
// until those callers exist.
#![allow(dead_code)]

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

/// Encodes everything except RFC-3986 unreserved characters (ASCII alphanumerics + `-._~`).
/// `NON_ALPHANUMERIC` encodes all non-alphanumerics; we remove the four unreserved punctuation
/// chars so they are not needlessly percent-encoded in the ScrapingBee `url` parameter.
const TARGET_URL_ENCODE_SET: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// How the caller should respond to a scrape failure.
#[derive(Debug, Clone, PartialEq)]
pub enum FailureClass {
    /// Non-retryable (e.g. 404/410 — page is gone). Mark the run `failed` immediately.
    Terminal,
    /// The target URL was not RFC-3986-percent-encoded (ScrapingBee returns
    /// `"Unknown arguments:"` in its body when query params leak into its own param parser).
    /// Re-issue once with the target URL correctly encoded; never retry again after that.
    FixEncoding,
    /// The target site blocked the request even through premium proxy.
    /// Re-enqueue once with `ProxyTier::Stealth`. If it still fails, mark `failed`.
    EscalateProxy,
    /// Transient (rate-limit, gateway error). Retry with bounded backoff (≤2 retries).
    Transient,
}

/// Which ScrapingBee proxy tier to use for this request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProxyTier {
    /// `premium_proxy=true`, 25 credits with JS rendering. Default for careers pages.
    Premium,
    /// `stealth_proxy=true`, 75 credits (JS rendering required). Escalation when Premium
    /// is blocked.
    Stealth,
}

/// A structured scrape failure: the HTTP status (if a response was received), the response body,
/// and the classified action the pipeline should take.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrapeError {
    pub status: Option<u16>,
    pub body: String,
    pub class: FailureClass,
}

impl std::fmt::Display for ScrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(s) => write!(f, "scrape failed ({s}): {}", self.body),
            None => write!(f, "scrape failed (no response): {}", self.body),
        }
    }
}

/// Classify a non-2xx response from ScrapingBee into the action the pipeline should take.
///
/// **Mapping (source: ScrapingBee KB "What to do if my request fails" + API docs):**
/// - 404 / 410 → `Terminal` (page is gone; retrying wastes credits)
/// - 429        → `Transient` (ScrapingBee rate limit; back off and retry)
/// - 503        → `Transient` (service unavailable; transient)
/// - 500 + body contains `"Unknown arguments:"` → `FixEncoding`
///   ScrapingBee parses the target URL's query parameters as its own API parameters when the
///   URL is not percent-encoded, returning `{"message":"Unknown arguments: <param>"}`.
/// - 500 (any other body), 403 → `EscalateProxy`
///   Indicates the target site returned an anti-bot/block response even through premium
///   proxy. The fix is stealth proxy.
/// - Everything else → `Transient` (conservative default — back off, retry)
pub fn classify_scrape_failure(status: u16, body: &str) -> FailureClass {
    match status {
        404 | 410 => FailureClass::Terminal,
        429 => FailureClass::Transient,
        503 => FailureClass::Transient,
        500 | 403 => {
            // ScrapingBee returns `{"message":"Unknown arguments: <param>"}` when the target
            // URL's query params weren't percent-encoded and leaked into the ScrapingBee
            // parser. This is fixable by re-issuing with a correctly-encoded URL.
            if body.contains("Unknown arguments:") {
                FailureClass::FixEncoding
            } else {
                // Any other 500/403 means the target site blocked us at the premium-proxy level.
                FailureClass::EscalateProxy
            }
        }
        _ => FailureClass::Transient,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScrapeResult {
    pub content: String,
    /// ScrapingBee credits charged, read from the `Spb-cost` response header
    /// ("Request cost in credits", per ScrapingBee's API-headers KB). `None` only if the
    /// header is absent/unparseable — recorded honestly, never a fabricated value.
    pub credits: Option<u32>,
}

pub trait Scraper {
    /// Fetch a URL's content using the specified proxy tier.
    /// `credits` reports ScrapingBee credits consumed (0 for fakes).
    fn fetch(&self, url: &str, tier: ProxyTier) -> Result<ScrapeResult, ScrapeError>;
}

const SCRAPINGBEE_ENDPOINT: &str = "https://app.scrapingbee.com/api/v1";

/// Percent-encode `url` per RFC-3986 so that any query parameters in the target URL
/// (e.g. `?a=b&c=d`) are encoded as `%3F`, `%3D`, `%26` rather than left as literal
/// `?`, `=`, `&` which ScrapingBee's parser would interpret as its own API parameters.
///
/// reqwest's `.query(&[("url", url)])` uses `form_urlencoded` which encodes spaces as `+`
/// (application/x-www-form-urlencoded, not RFC-3986). We bypass that and build the URL
/// string manually with `utf8_percent_encode` using `TARGET_URL_ENCODE_SET`, which encodes
/// everything except RFC-3986 unreserved characters (`[A-Za-z0-9-._~]`). This guarantees
/// `+` → `%2B`, `?` → `%3F`, `&` → `%26`, `=` → `%3D`, while keeping `-._~` unencoded.
pub fn percent_encode_target_url(url: &str) -> String {
    utf8_percent_encode(url, TARGET_URL_ENCODE_SET).to_string()
}

/// Real scraper: the ScrapingBee HTML API. **Premium proxy + JS rendering** by default —
/// ATS-backed careers pages (Greenhouse/Lever/Workday/Ashby…) are commonly bot-hardened, so
/// the cheap classic tier gets blocked; stealth is the next escalation if premium still fails.
/// Auth is `Authorization: Bearer <key>` (the query-param `api_key` is deprecated). Credits
/// charged are read from ScrapingBee's response cost header.
///
/// **Runtime constraint:** `reqwest::blocking` panics if started inside a tokio reactor, so the
/// pipeline runner must execute steps off-reactor (a sync Tauri command / dedicated worker
/// thread), NOT via `tokio::spawn`. Tasks 4 & 6 honor this.
pub struct ScrapingBeeScraper;

impl Scraper for ScrapingBeeScraper {
    fn fetch(&self, url: &str, tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
        let key = crate::secrets::get_secret("scrapingbee_api_key").map_err(|e| ScrapeError {
            status: None,
            body: e,
            class: FailureClass::Terminal,
        })?;
        // ScrapingBee with JS render + networkidle + the wait can take 20–30s+; the blocking
        // client's 30s DEFAULT timeout is too tight (the careers scrape already hit ~25s), so
        // set a generous explicit one.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .map_err(|e| ScrapeError {
                status: None,
                body: format!("scrapingbee client build failed: {e}"),
                class: FailureClass::Transient,
            })?;

        // Percent-encode the target URL per RFC-3986 so that any `?a=b&c=d` in it is
        // encoded as `%3Fa%3Db%26c%3Dd` and not mistaken for ScrapingBee API parameters.
        let encoded_url = percent_encode_target_url(url);

        // Build the request URL manually (bypass reqwest's form-urlencoding of `.query()`)
        // so we control the exact encoding of the `url` parameter.
        let proxy_param = match tier {
            ProxyTier::Premium => ("premium_proxy", "true"),
            ProxyTier::Stealth => ("stealth_proxy", "true"),
        };
        let request_url = format!(
            "{SCRAPINGBEE_ENDPOINT}?url={encoded_url}&render_js=true&{proxy_key}={proxy_val}\
             &wait_browser=networkidle2&wait=5000",
            proxy_key = proxy_param.0,
            proxy_val = proxy_param.1,
        );

        let resp = client
            .get(&request_url)
            .bearer_auth(&key)
            // ATS careers pages (Ashby/Greenhouse/Lever/…) are client-rendered SPAs that
            // fetch their listings via an async XHR *after* the initial render. Wait for the
            // network to settle so those listings are in the captured HTML, plus a small
            // buffer for the post-fetch DOM render. Without this, render_js returns the empty
            // shell and structure-listings finds nothing.
            .send()
            .map_err(|e| ScrapeError {
                status: None,
                body: format!("scrapingbee request failed: {e}"),
                class: FailureClass::Transient,
            })?;

        let status = resp.status();
        // Actual credits charged are in the `Spb-cost` response header ("Request cost in
        // credits", per ScrapingBee's API-headers KB; reqwest header lookup is case-insensitive).
        // `None` if absent/unparseable — the unknown is recorded honestly, not fabricated.
        let credits = resp
            .headers()
            .get("spb-cost")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u32>().ok());
        let body = resp.text().map_err(|e| ScrapeError {
            status: Some(status.as_u16()),
            body: format!("scrapingbee body read failed: {e}"),
            class: FailureClass::Transient,
        })?;

        if !status.is_success() {
            let status_u16 = status.as_u16();
            let class = classify_scrape_failure(status_u16, &body);
            return Err(ScrapeError {
                status: Some(status_u16),
                body,
                class,
            });
        }

        Ok(ScrapeResult {
            content: body,
            credits,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Test-only scraper that always succeeds with canned content.
    pub struct FakeScraper {
        pub content: String,
        pub credits: u32,
    }
    impl Scraper for FakeScraper {
        fn fetch(&self, _url: &str, _tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
            Ok(ScrapeResult {
                content: self.content.clone(),
                credits: Some(self.credits),
            })
        }
    }

    /// Test-only scraper that always fails with a specified `FailureClass`.
    pub struct FailingScraper {
        pub class: FailureClass,
        pub status: Option<u16>,
    }
    impl Scraper for FailingScraper {
        fn fetch(&self, _url: &str, _tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
            Err(ScrapeError {
                status: self.status,
                body: "fake failure".into(),
                class: self.class.clone(),
            })
        }
    }

    #[test]
    fn fake_returns_canned_content_and_credits() {
        let s = FakeScraper {
            content: "<p>x</p>".into(),
            credits: 5,
        };
        let r = s
            .fetch("https://acme.com/careers", ProxyTier::Premium)
            .unwrap();
        assert_eq!(r.content, "<p>x</p>");
        assert_eq!(r.credits, Some(5));
    }

    #[test]
    fn classifies_failures() {
        // Terminal: page is gone — retrying would waste credits
        assert_eq!(classify_scrape_failure(404, ""), FailureClass::Terminal);
        assert_eq!(classify_scrape_failure(410, ""), FailureClass::Terminal);
        // Transient: rate limit / service unavailable
        assert_eq!(classify_scrape_failure(429, ""), FailureClass::Transient);
        assert_eq!(classify_scrape_failure(503, ""), FailureClass::Transient);
        // body-driven 500: encoding problem — ScrapingBee parses unencoded URL params as its own
        assert_eq!(
            classify_scrape_failure(500, r#"{"message":"Unknown arguments: foo"}"#),
            FailureClass::FixEncoding
        );
        // body-driven 500: anything else means anti-bot block → escalate proxy
        assert_eq!(
            classify_scrape_failure(500, "<html>Access Denied</html>"),
            FailureClass::EscalateProxy
        );
        // 403 with no encoding signal → block → escalate
        assert_eq!(
            classify_scrape_failure(403, "<html>Forbidden</html>"),
            FailureClass::EscalateProxy
        );
    }

    #[test]
    fn percent_encode_target_url_encodes_query_params() {
        // A target URL with query params must be encoded so that `?`, `=`, `&`, and `+`
        // are not parsed as ScrapingBee API parameters.
        let raw = "https://example.com/jobs?location=New+York&type=full-time";
        let encoded = percent_encode_target_url(raw);
        // `+` must become `%2B`, `?` → `%3F`, `&` → `%26`, `=` → `%3D`
        assert!(
            encoded.contains("%2B"),
            "'+' must be encoded as '%2B', got: {encoded}"
        );
        assert!(
            encoded.contains("%3F"),
            "'?' must be encoded as '%3F', got: {encoded}"
        );
        assert!(
            encoded.contains("%26"),
            "'&' must be encoded as '%26', got: {encoded}"
        );
        assert!(
            encoded.contains("%3D"),
            "'=' must be encoded as '%3D', got: {encoded}"
        );
        assert!(
            !encoded.contains('+'),
            "bare '+' must not remain, got: {encoded}"
        );
        assert!(
            !encoded.contains('?'),
            "bare '?' must not remain, got: {encoded}"
        );
        // RFC-3986 unreserved chars `-._~` must NOT be percent-encoded (Fix 2).
        let unreserved = percent_encode_target_url("a-b.c_d~e");
        assert_eq!(
            unreserved, "a-b.c_d~e",
            "RFC-3986 unreserved chars must pass through unchanged, got: {unreserved}"
        );
    }
}
