//! `region-adaptation` — typed [`RegionProfile`] bundling
//! compliance regime + payment providers + search engines per
//! ISO 3166-1 region.
//!
//! Per `PLATFORM_ROADMAP.md` §7 + the "ready for everyone
//! globally" framing: a site that ships to a Chinese audience
//! needs Baidu sitemap submission + Alipay/WeChat Pay rendering
//! + PIPL compliance markers — not Google + Stripe + GDPR.
//! Region adaptation is a first-class platform primitive,
//! enumerated as a closed type so consumers can't drift.
//!
//! ### What this crate ships
//!
//! Three orthogonal closed enums + a bundling profile:
//!
//!   * [`ComplianceRegime`]  — which data-protection law applies
//!     (GDPR / CCPA / LGPD / PIPL / POPIA / APP / PIPEDA / KVKK / …)
//!   * [`PaymentProvider`]   — which payment rails work in-region
//!     (Stripe / Alipay / WeChatPay / MercadoPago / Razorpay / …)
//!   * [`SearchEngine`]      — which search index dominates
//!     (Google / Baidu / Yandex / Naver / Seznam / …)
//!   * [`RegionProfile`]     — combines a [`RegionId`] (ISO 3166-1
//!     alpha-2) with the active regime + accepted payment rails +
//!     primary/secondary search engines.
//!
//! Plus presets ([`RegionProfile::preset`]) for the major markets
//! the platform supports out-of-the-box. Operators in other
//! regions register their own profile.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Typed ISO 3166-1 alpha-2 region identifier (e.g. `"US"`,
/// `"CN"`, `"DE"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegionId(String);

impl RegionId {
    /// Construct from a 2-letter ISO 3166-1 alpha-2 code.
    /// Uppercase ASCII letters only.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, RegionError> {
        let s = s.as_ref();
        if s.len() != 2 || !s.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(RegionError::InvalidId(format!(
                "{s:?} not an ISO 3166-1 alpha-2 code"
            )));
        }
        Ok(Self(s.to_string()))
    }

    /// Raw 2-letter code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RegionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Data-protection regime applicable in a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplianceRegime {
    /// European Union GDPR + ePrivacy Directive.
    Gdpr,
    /// California CCPA / CPRA.
    Ccpa,
    /// Brazilian LGPD.
    Lgpd,
    /// Chinese PIPL.
    Pipl,
    /// South African POPIA.
    Popia,
    /// Australian Privacy Act / APP.
    App,
    /// Canadian PIPEDA.
    Pipeda,
    /// Turkish KVKK.
    Kvkk,
    /// Russian Federal Law on Personal Data (152-FZ).
    #[serde(rename = "ru-152fz")]
    Ru152fz,
    /// Korean PIPA.
    KrPipa,
    /// Japanese APPI.
    JpAppi,
    /// India DPDP Act 2023.
    InDpdp,
    /// UK Data Protection Act 2018 / UK GDPR.
    UkDpa,
    /// No regulated regime (operator-declared "ungoverned").
    /// Use sparingly — most jurisdictions have at least some
    /// consumer-protection baseline.
    None,
}

impl ComplianceRegime {
    /// Stable kebab-case slug for serialization + admin UI.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Gdpr => "gdpr",
            Self::Ccpa => "ccpa",
            Self::Lgpd => "lgpd",
            Self::Pipl => "pipl",
            Self::Popia => "popia",
            Self::App => "app",
            Self::Pipeda => "pipeda",
            Self::Kvkk => "kvkk",
            Self::Ru152fz => "ru-152fz",
            Self::KrPipa => "kr-pipa",
            Self::JpAppi => "jp-appi",
            Self::InDpdp => "in-dpdp",
            Self::UkDpa => "uk-dpa",
            Self::None => "none",
        }
    }

    /// Whether this regime requires an explicit cookie consent
    /// banner with opt-in (rather than opt-out).
    pub fn requires_cookie_optin(&self) -> bool {
        matches!(
            self,
            Self::Gdpr
                | Self::UkDpa
                | Self::Lgpd
                | Self::Pipl
                | Self::Pipeda
                | Self::KrPipa
                | Self::JpAppi
                | Self::InDpdp
                | Self::Kvkk
        )
    }

    /// Whether this regime requires a "Do Not Sell" / opt-out
    /// path for personal information sales.
    pub fn requires_do_not_sell(&self) -> bool {
        matches!(self, Self::Ccpa)
    }
}

/// Payment provider accepted in a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaymentProvider {
    /// Stripe (US / EU / UK / AU / SG / JP / global card rails).
    Stripe,
    /// PayPal.
    Paypal,
    /// Alipay (China primary).
    Alipay,
    /// WeChat Pay (China primary).
    WechatPay,
    /// UnionPay (China card rail).
    Unionpay,
    /// MercadoPago (LatAm).
    Mercadopago,
    /// Razorpay (India).
    Razorpay,
    /// Paytm (India).
    Paytm,
    /// Klarna (EU + US — buy-now-pay-later).
    Klarna,
    /// iDEAL (Netherlands).
    Ideal,
    /// SEPA Direct Debit (EU).
    Sepa,
    /// Bank transfer (region-generic).
    BankTransfer,
    /// Apple Pay.
    ApplePay,
    /// Google Pay.
    GooglePay,
    /// Yandex.Money / YooMoney (Russia).
    #[serde(rename = "yoomoney")]
    YooMoney,
    /// PIX (Brazil instant payment).
    Pix,
}

impl PaymentProvider {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Stripe => "stripe",
            Self::Paypal => "paypal",
            Self::Alipay => "alipay",
            Self::WechatPay => "wechat-pay",
            Self::Unionpay => "unionpay",
            Self::Mercadopago => "mercadopago",
            Self::Razorpay => "razorpay",
            Self::Paytm => "paytm",
            Self::Klarna => "klarna",
            Self::Ideal => "ideal",
            Self::Sepa => "sepa",
            Self::BankTransfer => "bank-transfer",
            Self::ApplePay => "apple-pay",
            Self::GooglePay => "google-pay",
            Self::YooMoney => "yoomoney",
            Self::Pix => "pix",
        }
    }
}

/// Search engine that indexes the region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchEngine {
    /// Google (global default).
    Google,
    /// Bing (global secondary; primary on some Windows defaults).
    Bing,
    /// DuckDuckGo (privacy-first).
    Duckduckgo,
    /// Baidu (China primary).
    Baidu,
    /// Yandex (Russia, Belarus, Kazakhstan).
    Yandex,
    /// Naver (South Korea primary).
    Naver,
    /// Daum / Kakao (South Korea secondary).
    Daum,
    /// Seznam (Czech Republic).
    Seznam,
    /// Yahoo Japan (Japan secondary).
    YahooJp,
    /// Sogou / 360 (China secondary).
    Sogou,
    /// Ecosia (privacy + environment).
    Ecosia,
    /// Qwant (privacy-first, EU).
    Qwant,
    /// Kagi (paid privacy-first).
    Kagi,
    /// Mojeek (independent index).
    Mojeek,
}

impl SearchEngine {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Bing => "bing",
            Self::Duckduckgo => "duckduckgo",
            Self::Baidu => "baidu",
            Self::Yandex => "yandex",
            Self::Naver => "naver",
            Self::Daum => "daum",
            Self::Seznam => "seznam",
            Self::YahooJp => "yahoo-jp",
            Self::Sogou => "sogou",
            Self::Ecosia => "ecosia",
            Self::Qwant => "qwant",
            Self::Kagi => "kagi",
            Self::Mojeek => "mojeek",
        }
    }

    /// Sitemap submission endpoint, when the engine accepts one.
    /// `None` for engines that don't expose a public submission
    /// URL or that mandate an authenticated webmaster console
    /// only.
    pub fn sitemap_endpoint(&self) -> Option<&'static str> {
        match self {
            Self::Google => Some("https://www.google.com/ping?sitemap="),
            Self::Bing => Some("https://www.bing.com/ping?sitemap="),
            Self::Yandex => Some("https://yandex.com/indexnow?url="),
            Self::Seznam => Some("https://search.seznam.cz/sitemap?url="),
            // Baidu requires authenticated submission via Baidu
            // Search Console; no public ping endpoint.
            Self::Baidu => None,
            Self::Naver => None,
            // Privacy engines don't accept submission.
            Self::Duckduckgo | Self::Qwant | Self::Kagi | Self::Mojeek | Self::Ecosia => None,
            Self::Daum | Self::YahooJp | Self::Sogou => None,
        }
    }
}

/// Combined region profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RegionProfile {
    /// ISO 3166-1 alpha-2 region code.
    pub region: RegionId,
    /// Data-protection regime in force.
    pub compliance: ComplianceRegime,
    /// Payment providers the site should render (in display order).
    pub payments: Vec<PaymentProvider>,
    /// Search engines the site should target (first = primary).
    pub search: Vec<SearchEngine>,
}

impl RegionProfile {
    /// Return a curated preset for a known region. `None` means
    /// the platform doesn't ship a preset yet — operator should
    /// declare one explicitly.
    pub fn preset(region: &RegionId) -> Option<Self> {
        let p = match region.as_str() {
            "US" => Self {
                region: RegionId::parse("US").unwrap(),
                compliance: ComplianceRegime::Ccpa,
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::ApplePay,
                    PaymentProvider::GooglePay,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::Bing,
                    SearchEngine::Duckduckgo,
                ],
            },
            "GB" | "IE" => Self {
                region: region.clone(),
                compliance: if region.as_str() == "GB" {
                    ComplianceRegime::UkDpa
                } else {
                    ComplianceRegime::Gdpr
                },
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::Klarna,
                    PaymentProvider::ApplePay,
                    PaymentProvider::GooglePay,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::Bing,
                    SearchEngine::Duckduckgo,
                ],
            },
            "DE" | "FR" | "ES" | "IT" | "NL" | "BE" | "PL" | "SE" | "DK" | "FI" | "NO" | "AT"
            | "PT" | "GR" | "IE_EU" => {
                let mut payments = vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::Sepa,
                    PaymentProvider::Klarna,
                ];
                if region.as_str() == "NL" {
                    payments.push(PaymentProvider::Ideal);
                }
                Self {
                    region: region.clone(),
                    compliance: ComplianceRegime::Gdpr,
                    payments,
                    search: vec![
                        SearchEngine::Google,
                        SearchEngine::Bing,
                        SearchEngine::Qwant,
                    ],
                }
            }
            "CZ" => Self {
                region: RegionId::parse("CZ").unwrap(),
                compliance: ComplianceRegime::Gdpr,
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::Sepa,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::Seznam,
                    SearchEngine::Bing,
                ],
            },
            "CN" => Self {
                region: RegionId::parse("CN").unwrap(),
                compliance: ComplianceRegime::Pipl,
                payments: vec![
                    PaymentProvider::Alipay,
                    PaymentProvider::WechatPay,
                    PaymentProvider::Unionpay,
                ],
                search: vec![SearchEngine::Baidu, SearchEngine::Sogou],
            },
            "RU" => Self {
                region: RegionId::parse("RU").unwrap(),
                compliance: ComplianceRegime::Ru152fz,
                payments: vec![PaymentProvider::YooMoney, PaymentProvider::BankTransfer],
                search: vec![SearchEngine::Yandex, SearchEngine::Google],
            },
            "BR" => Self {
                region: RegionId::parse("BR").unwrap(),
                compliance: ComplianceRegime::Lgpd,
                payments: vec![
                    PaymentProvider::Pix,
                    PaymentProvider::Mercadopago,
                    PaymentProvider::Stripe,
                ],
                search: vec![SearchEngine::Google, SearchEngine::Bing],
            },
            "JP" => Self {
                region: RegionId::parse("JP").unwrap(),
                compliance: ComplianceRegime::JpAppi,
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::ApplePay,
                    PaymentProvider::Paypal,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::YahooJp,
                    SearchEngine::Bing,
                ],
            },
            "KR" => Self {
                region: RegionId::parse("KR").unwrap(),
                compliance: ComplianceRegime::KrPipa,
                payments: vec![PaymentProvider::Stripe, PaymentProvider::Paypal],
                search: vec![
                    SearchEngine::Naver,
                    SearchEngine::Daum,
                    SearchEngine::Google,
                ],
            },
            "IN" => Self {
                region: RegionId::parse("IN").unwrap(),
                compliance: ComplianceRegime::InDpdp,
                payments: vec![
                    PaymentProvider::Razorpay,
                    PaymentProvider::Paytm,
                    PaymentProvider::Stripe,
                ],
                search: vec![SearchEngine::Google, SearchEngine::Bing],
            },
            "AU" => Self {
                region: RegionId::parse("AU").unwrap(),
                compliance: ComplianceRegime::App,
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::ApplePay,
                    PaymentProvider::GooglePay,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::Bing,
                    SearchEngine::Duckduckgo,
                ],
            },
            "CA" => Self {
                region: RegionId::parse("CA").unwrap(),
                compliance: ComplianceRegime::Pipeda,
                payments: vec![
                    PaymentProvider::Stripe,
                    PaymentProvider::Paypal,
                    PaymentProvider::ApplePay,
                ],
                search: vec![
                    SearchEngine::Google,
                    SearchEngine::Bing,
                    SearchEngine::Duckduckgo,
                ],
            },
            "ZA" => Self {
                region: RegionId::parse("ZA").unwrap(),
                compliance: ComplianceRegime::Popia,
                payments: vec![PaymentProvider::Stripe, PaymentProvider::Paypal],
                search: vec![SearchEngine::Google, SearchEngine::Bing],
            },
            "TR" => Self {
                region: RegionId::parse("TR").unwrap(),
                compliance: ComplianceRegime::Kvkk,
                payments: vec![PaymentProvider::Stripe, PaymentProvider::Paypal],
                search: vec![SearchEngine::Google, SearchEngine::Yandex],
            },
            "IL" => Self {
                region: RegionId::parse("IL").unwrap(),
                compliance: ComplianceRegime::None,
                payments: vec![PaymentProvider::Stripe, PaymentProvider::Paypal],
                search: vec![SearchEngine::Google, SearchEngine::Bing],
            },
            "IR" => Self {
                region: RegionId::parse("IR").unwrap(),
                compliance: ComplianceRegime::None,
                payments: vec![PaymentProvider::BankTransfer],
                search: vec![SearchEngine::Google, SearchEngine::Yandex],
            },
            _ => return None,
        };
        Some(p)
    }

    /// Convenience: build a sitemap-ping URL for every search
    /// engine in `self.search` that exposes a public submission
    /// endpoint. `site_sitemap_url` is the URL of the operator's
    /// own sitemap.xml.
    pub fn sitemap_ping_urls(&self, site_sitemap_url: &str) -> Vec<String> {
        self.search
            .iter()
            .filter_map(|e| e.sitemap_endpoint())
            .map(|endpoint| format!("{endpoint}{}", url_encode(site_sitemap_url)))
            .collect()
    }
}

/// Errors at the region-adaptation boundary.
#[derive(Debug, thiserror::Error)]
pub enum RegionError {
    /// RegionId failed validation.
    #[error("invalid region id: {0}")]
    InvalidId(String),
}

/// Minimal URL-encoding for sitemap ping URLs. Encodes the
/// reserved + space characters; full RFC 3986 encoding is
/// overkill for sitemap URLs which only ever contain ASCII +
/// digits + `/-_.`.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_id_validates() {
        assert!(RegionId::parse("US").is_ok());
        assert!(RegionId::parse("CN").is_ok());
        assert!(RegionId::parse("us").is_err());
        assert!(RegionId::parse("USA").is_err());
        assert!(RegionId::parse("U").is_err());
        assert!(RegionId::parse("").is_err());
    }

    #[test]
    fn compliance_cookie_optin_set_matches_known_regimes() {
        assert!(ComplianceRegime::Gdpr.requires_cookie_optin());
        assert!(ComplianceRegime::UkDpa.requires_cookie_optin());
        assert!(ComplianceRegime::Lgpd.requires_cookie_optin());
        assert!(ComplianceRegime::Pipl.requires_cookie_optin());
        assert!(!ComplianceRegime::Ccpa.requires_cookie_optin());
        assert!(!ComplianceRegime::None.requires_cookie_optin());
    }

    #[test]
    fn ccpa_requires_do_not_sell() {
        assert!(ComplianceRegime::Ccpa.requires_do_not_sell());
        assert!(!ComplianceRegime::Gdpr.requires_do_not_sell());
        assert!(!ComplianceRegime::Pipl.requires_do_not_sell());
    }

    #[test]
    fn slugs_are_distinct_within_each_enum() {
        let regimes = [
            ComplianceRegime::Gdpr,
            ComplianceRegime::Ccpa,
            ComplianceRegime::Lgpd,
            ComplianceRegime::Pipl,
            ComplianceRegime::Popia,
            ComplianceRegime::App,
            ComplianceRegime::Pipeda,
            ComplianceRegime::Kvkk,
            ComplianceRegime::Ru152fz,
            ComplianceRegime::KrPipa,
            ComplianceRegime::JpAppi,
            ComplianceRegime::InDpdp,
            ComplianceRegime::UkDpa,
            ComplianceRegime::None,
        ];
        let mut seen = std::collections::HashSet::new();
        for r in regimes {
            assert!(seen.insert(r.slug()), "duplicate regime slug {}", r.slug());
        }
    }

    #[test]
    fn presets_for_major_regions() {
        for code in [
            "US", "GB", "DE", "FR", "CN", "RU", "BR", "JP", "KR", "IN", "AU", "CA", "ZA", "TR",
        ] {
            let r = RegionId::parse(code).unwrap();
            let p = RegionProfile::preset(&r).expect(code);
            assert_eq!(p.region.as_str(), code);
            assert!(!p.payments.is_empty(), "{code} payments");
            assert!(!p.search.is_empty(), "{code} search");
        }
    }

    #[test]
    fn cn_preset_uses_baidu_and_alipay() {
        let r = RegionId::parse("CN").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        assert_eq!(p.compliance, ComplianceRegime::Pipl);
        assert!(p.payments.contains(&PaymentProvider::Alipay));
        assert!(p.payments.contains(&PaymentProvider::WechatPay));
        assert_eq!(p.search[0], SearchEngine::Baidu);
    }

    #[test]
    fn ru_preset_uses_yandex_and_152fz() {
        let r = RegionId::parse("RU").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        assert_eq!(p.compliance, ComplianceRegime::Ru152fz);
        assert_eq!(p.search[0], SearchEngine::Yandex);
    }

    #[test]
    fn kr_preset_uses_naver_then_daum_then_google() {
        let r = RegionId::parse("KR").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        assert_eq!(p.search[0], SearchEngine::Naver);
        assert_eq!(p.search[1], SearchEngine::Daum);
    }

    #[test]
    fn br_preset_uses_pix_and_lgpd() {
        let r = RegionId::parse("BR").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        assert_eq!(p.compliance, ComplianceRegime::Lgpd);
        assert_eq!(p.payments[0], PaymentProvider::Pix);
    }

    #[test]
    fn us_preset_uses_ccpa_not_gdpr() {
        let r = RegionId::parse("US").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        assert_eq!(p.compliance, ComplianceRegime::Ccpa);
        assert!(p.compliance.requires_do_not_sell());
        assert!(!p.compliance.requires_cookie_optin());
    }

    #[test]
    fn unknown_region_returns_none_preset() {
        let r = RegionId::parse("XX").unwrap();
        assert!(RegionProfile::preset(&r).is_none());
    }

    #[test]
    fn search_engine_sitemap_endpoint_known() {
        assert!(SearchEngine::Google.sitemap_endpoint().is_some());
        assert!(SearchEngine::Bing.sitemap_endpoint().is_some());
        assert!(SearchEngine::Yandex.sitemap_endpoint().is_some());
        assert!(SearchEngine::Baidu.sitemap_endpoint().is_none());
        assert!(SearchEngine::Naver.sitemap_endpoint().is_none());
        assert!(SearchEngine::Duckduckgo.sitemap_endpoint().is_none());
    }

    #[test]
    fn sitemap_ping_urls_compose_correctly() {
        let r = RegionId::parse("US").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        let urls = p.sitemap_ping_urls("https://example.com/sitemap.xml");
        assert_eq!(urls.len(), 2); // google + bing have endpoints, duckduckgo doesn't
        assert!(urls[0].contains("google.com/ping"));
        assert!(urls[0].contains("https%3A%2F%2Fexample.com%2Fsitemap.xml"));
    }

    #[test]
    fn region_profile_serde_round_trips() {
        let r = RegionId::parse("DE").unwrap();
        let p = RegionProfile::preset(&r).unwrap();
        let s = serde_json::to_string(&p).unwrap();
        let back: RegionProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn region_profile_rejects_unknown_field() {
        let bad = r#"{"region":"US","compliance":"ccpa","payments":[],"search":[],"ahem":1}"#;
        let r: Result<RegionProfile, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn url_encode_handles_unicode_and_reserved() {
        assert_eq!(url_encode("a b"), "a%20b");
        assert_eq!(url_encode("a/b"), "a%2Fb");
        assert_eq!(url_encode("a-_.~"), "a-_.~");
    }

    // T97: slug-vs-serde-wire regression guard.
    // ComplianceRegime + PaymentProvider have many variants;
    // Ru152fz (digit boundary) is a known-bug candidate; some
    // payment providers (YooMoney etc.) likely have CamelCase
    // boundaries the kebab transform splits differently from
    // the slug.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            ComplianceRegime::Gdpr,
            ComplianceRegime::Ccpa,
            ComplianceRegime::Lgpd,
            ComplianceRegime::Pipl,
            ComplianceRegime::Popia,
            ComplianceRegime::App,
            ComplianceRegime::Pipeda,
            ComplianceRegime::Kvkk,
            ComplianceRegime::Ru152fz,
            ComplianceRegime::KrPipa,
            ComplianceRegime::JpAppi,
            ComplianceRegime::InDpdp,
            ComplianceRegime::UkDpa,
            ComplianceRegime::None,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            PaymentProvider::Stripe,
            PaymentProvider::Paypal,
            PaymentProvider::Alipay,
            PaymentProvider::WechatPay,
            PaymentProvider::Unionpay,
            PaymentProvider::Mercadopago,
            PaymentProvider::Razorpay,
            PaymentProvider::Paytm,
            PaymentProvider::Klarna,
            PaymentProvider::Ideal,
            PaymentProvider::Sepa,
            PaymentProvider::BankTransfer,
            PaymentProvider::ApplePay,
            PaymentProvider::GooglePay,
            PaymentProvider::YooMoney,
            PaymentProvider::Pix,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SearchEngine::Google,
            SearchEngine::Bing,
            SearchEngine::Duckduckgo,
            SearchEngine::Baidu,
            SearchEngine::Yandex,
            SearchEngine::Naver,
            SearchEngine::Daum,
            SearchEngine::Seznam,
            SearchEngine::YahooJp,
            SearchEngine::Sogou,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
