//! The scraping seam. The app always wires the real `ScrapingBeeScraper`; `FakeScraper`
//! is test-only. `fetch` returns the page content + the ScrapingBee credits consumed.
// Trait + result type are public API consumed by the pipeline (Tasks 4–6); the real
// `ScrapingBeeScraper` lands with Task 6 (ScrapingBee Request Builder). Suppress dead-code
// until those callers exist.
#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct ScrapeResult {
    pub content: String,
    /// ScrapingBee credits charged, read from the `Spb-cost` response header
    /// ("Request cost in credits", per ScrapingBee's API-headers KB). `None` only if the
    /// header is absent/unparseable — recorded honestly, never a fabricated value.
    pub credits: Option<u32>,
}

pub trait Scraper {
    /// Fetch a URL's content. `credits` reports ScrapingBee credits consumed (0 for fakes).
    fn fetch(&self, url: &str) -> Result<ScrapeResult, String>;
}

const SCRAPINGBEE_ENDPOINT: &str = "https://app.scrapingbee.com/api/v1";

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
    fn fetch(&self, url: &str) -> Result<ScrapeResult, String> {
        let key = crate::secrets::get_secret("scrapingbee_api_key")?;
        // ScrapingBee with JS render + networkidle + the wait can take 20–30s+; the blocking
        // client's 30s DEFAULT timeout is too tight (the careers scrape already hit ~25s), so
        // set a generous explicit one.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .map_err(|e| format!("scrapingbee client build failed: {e}"))?;
        let resp = client
            .get(SCRAPINGBEE_ENDPOINT)
            .bearer_auth(&key)
            .query(&[
                ("url", url),
                ("render_js", "true"),
                ("premium_proxy", "true"),
                // ATS careers pages (Ashby/Greenhouse/Lever/…) are client-rendered SPAs that
                // fetch their listings via an async XHR *after* the initial render. Wait for the
                // network to settle so those listings are in the captured HTML, plus a small
                // buffer for the post-fetch DOM render. Without this, render_js returns the empty
                // shell and structure-listings finds nothing.
                ("wait_browser", "networkidle2"),
                ("wait", "5000"),
            ])
            .send()
            .map_err(|e| format!("scrapingbee request failed: {e}"))?;
        let status = resp.status();
        // Actual credits charged are in the `Spb-cost` response header ("Request cost in
        // credits", per ScrapingBee's API-headers KB; reqwest header lookup is case-insensitive).
        // `None` if absent/unparseable — the unknown is recorded honestly, not fabricated.
        let credits = resp
            .headers()
            .get("spb-cost")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u32>().ok());
        let body = resp
            .text()
            .map_err(|e| format!("scrapingbee body read failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("scrapingbee returned {status}: {body}"));
        }
        Ok(ScrapeResult { content: body, credits })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Test-only scraper. Reachable cross-module as `crate::scraper::tests::FakeScraper`.
    pub struct FakeScraper {
        pub content: String,
        pub credits: u32,
    }
    impl Scraper for FakeScraper {
        fn fetch(&self, _url: &str) -> Result<ScrapeResult, String> {
            Ok(ScrapeResult { content: self.content.clone(), credits: Some(self.credits) })
        }
    }

    #[test]
    fn fake_returns_canned_content_and_credits() {
        let s = FakeScraper { content: "<p>x</p>".into(), credits: 5 };
        let r = s.fetch("https://acme.com/careers").unwrap();
        assert_eq!(r.content, "<p>x</p>");
        assert_eq!(r.credits, Some(5));
    }
}
