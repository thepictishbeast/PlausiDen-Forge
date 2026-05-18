//! `commerce-storefront-core` — typed tenant-storefront contract.
//!
//! Per `PLATFORM_ROADMAP.md` §18, every PlausiDen tenant can sell
//! to *their* customers. This crate defines the tenant-storefront
//! shape — Product + Variant + Inventory + TaxJurisdiction +
//! ShippingZone + customer Subscription — distinct from
//! `commerce-core` (T72), which covers the operator's own
//! PlausiDen billing.
//!
//! Naming convention:
//!   * `commerce-core`            — what PlausiDen charges the tenant
//!   * `commerce-storefront-core` — what the tenant charges their customers
//!
//! ### Why typed
//!
//! E-commerce drift kills tenants. "We thought EU VAT was 21% but
//! shipped at the seller's rate." "Inventory was 0 but the
//! checkout completed because the variant was cached." Closed
//! enums + reservation-state machine prevent the most common
//! storefront integrity bugs at the type-checker, not at
//! audit time.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Currency code (ISO 4217 3-letter). New-typed so a price-in-EUR
/// can't accidentally be added to a price-in-USD.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    /// Construct, validating ISO 4217 shape: exactly 3 uppercase
    /// ASCII letters.
    pub fn new(code: impl Into<String>) -> Result<Self, CommerceError> {
        let c = code.into();
        if c.len() != 3 || !c.chars().all(|x| x.is_ascii_uppercase()) {
            return Err(CommerceError::Invalid(format!(
                "currency not ISO 4217 3-upper-letter: {}",
                c
            )));
        }
        Ok(Self(c))
    }

    /// String access.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Monetary amount in the smallest unit of the currency (cents
/// for USD/EUR/GBP; yen for JPY; etc.). i64 prevents accidental
/// float rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(pub i64);

/// Product — one sellable item, before variant selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Product {
    /// Stable product id (operator-defined; tenant scope).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Variant list — at least one required.
    pub variants: Vec<Variant>,
    /// Whether the product is published (visible on storefront).
    pub published: bool,
}

impl Product {
    /// Validate that the product has at least one variant.
    pub fn validate(&self) -> Result<(), CommerceError> {
        if self.title.trim().is_empty() {
            return Err(CommerceError::Invalid("title empty".into()));
        }
        if self.variants.is_empty() {
            return Err(CommerceError::Invalid(format!(
                "product {} has no variants",
                self.id
            )));
        }
        for v in &self.variants {
            v.validate()?;
        }
        Ok(())
    }

    /// Look up a variant by id.
    pub fn variant(&self, id: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.id == id)
    }
}

/// Variant — one (product, options) pair the customer can buy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Variant {
    /// Stable variant id.
    pub id: String,
    /// SKU.
    pub sku: String,
    /// Display title (e.g. "Medium / Blue").
    pub title: String,
    /// Unit price.
    pub price: Money,
    /// Currency.
    pub currency: CurrencyCode,
    /// Inventory snapshot — `None` means "unlimited".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory_on_hand: Option<u32>,
    /// Weight in grams, for shipping calc. `None` = digital.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_grams: Option<u32>,
}

impl Variant {
    /// Validate the variant. Price ≥ 0, sku non-empty, currency
    /// is ISO 4217 3-upper-letter.
    ///
    /// Note: CurrencyCode's `#[serde(transparent)]` means the
    /// inner String is deserialised directly without running
    /// `CurrencyCode::new()`. So validate() re-runs the shape
    /// check here — operator-supplied TOML can supply
    /// "usd"/"US"/"USDA" and the constructor never sees it.
    pub fn validate(&self) -> Result<(), CommerceError> {
        if self.sku.trim().is_empty() {
            return Err(CommerceError::Invalid(format!(
                "variant {} sku empty",
                self.id
            )));
        }
        if self.price.0 < 0 {
            return Err(CommerceError::Invalid(format!(
                "variant {} price negative: {}",
                self.id, self.price.0
            )));
        }
        let c = self.currency.as_str();
        if c.len() != 3 || !c.chars().all(|x| x.is_ascii_uppercase()) {
            return Err(CommerceError::Invalid(format!(
                "variant {} currency not ISO 4217 3-upper-letter: {:?}",
                self.id, c
            )));
        }
        Ok(())
    }

    /// Whether this variant is in stock for a quantity request.
    /// Unlimited inventory always returns true.
    pub fn has_stock_for(&self, qty: u32) -> bool {
        match self.inventory_on_hand {
            None => true,
            Some(on_hand) => on_hand >= qty,
        }
    }
}

/// Inventory reservation lifecycle. Reserving prevents
/// over-selling during a checkout that takes seconds to complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReservationState {
    /// Held — counted against on-hand for a short TTL while the
    /// customer completes checkout.
    Held,
    /// Committed — the order succeeded; on-hand is decremented.
    Committed,
    /// Released — the customer abandoned / failed; the hold
    /// expired without commit.
    Released,
}

impl ReservationState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Committed => "committed",
            Self::Released => "released",
        }
    }

    /// Whether the state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed | Self::Released)
    }
}

/// Tax jurisdiction. Closed enum prevents the operator from
/// silently treating an EU sale as a US sale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaxJurisdiction {
    /// EU — VAT under the OSS/IOSS scheme. Rate depends on
    /// destination member state.
    EuVat,
    /// UK — VAT under HMRC rules post-Brexit.
    UkVat,
    /// US — sales tax under state nexus rules.
    UsSalesTax,
    /// Canada — GST/HST/PST.
    CaGst,
    /// Australia — GST under ATO rules.
    AuGst,
    /// No tax applies (digital export to non-applicable
    /// jurisdiction; operator's risk to verify).
    None,
}

impl TaxJurisdiction {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::EuVat => "eu-vat",
            Self::UkVat => "uk-vat",
            Self::UsSalesTax => "us-sales-tax",
            Self::CaGst => "ca-gst",
            Self::AuGst => "au-gst",
            Self::None => "none",
        }
    }
}

/// One shipping zone — a country / region group with a flat
/// rate or weight-tier table operator-managed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ShippingZone {
    /// Stable zone id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// ISO 3166-1 alpha-2 country codes covered by this zone.
    pub countries: Vec<String>,
    /// Base rate (smallest currency unit).
    pub base_rate: Money,
    /// Currency.
    pub currency: CurrencyCode,
}

impl ShippingZone {
    /// Whether the zone covers a given ISO 3166-1 alpha-2 country
    /// code.
    pub fn covers(&self, country_a2: &str) -> bool {
        self.countries.iter().any(|c| c == country_a2)
    }
}

/// Customer-facing subscription cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionCadence {
    /// Weekly.
    Weekly,
    /// Monthly.
    Monthly,
    /// Quarterly.
    Quarterly,
    /// Annual.
    Annual,
}

impl SubscriptionCadence {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Annual => "annual",
        }
    }

    /// Approximate billing interval in days.
    pub fn approx_days(&self) -> u32 {
        match self {
            Self::Weekly => 7,
            Self::Monthly => 30,
            Self::Quarterly => 91,
            Self::Annual => 365,
        }
    }
}

/// Typed errors at the storefront boundary.
#[derive(Debug, thiserror::Error)]
pub enum CommerceError {
    /// Invalid data at construction.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Inventory insufficient.
    #[error("out-of-stock: variant {variant} requested {requested}")]
    OutOfStock {
        /// Variant id.
        variant: String,
        /// Requested quantity.
        requested: u32,
    },
    /// Backend / IO error.
    #[error("backend: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd() -> CurrencyCode {
        CurrencyCode::new("USD").unwrap()
    }

    fn variant_ok() -> Variant {
        Variant {
            id: "v1".into(),
            sku: "SKU-1".into(),
            title: "M / Blue".into(),
            price: Money(1999),
            currency: usd(),
            inventory_on_hand: Some(10),
            weight_grams: Some(250),
        }
    }

    fn product_ok() -> Product {
        Product {
            id: "p1".into(),
            title: "T-Shirt".into(),
            description: "Cotton".into(),
            variants: vec![variant_ok()],
            published: true,
        }
    }

    #[test]
    fn currency_validates_iso4217_shape() {
        assert!(CurrencyCode::new("USD").is_ok());
        assert!(CurrencyCode::new("EUR").is_ok());
        assert!(CurrencyCode::new("usd").is_err());
        assert!(CurrencyCode::new("US").is_err());
        assert!(CurrencyCode::new("USDA").is_err());
        assert!(CurrencyCode::new("US1").is_err());
    }

    #[test]
    fn product_validate_happy_path() {
        assert!(product_ok().validate().is_ok());
    }

    #[test]
    fn product_rejects_empty_title() {
        let mut p = product_ok();
        p.title = "".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn product_rejects_no_variants() {
        let mut p = product_ok();
        p.variants.clear();
        assert!(p.validate().is_err());
    }

    #[test]
    fn product_variant_lookup() {
        let p = product_ok();
        assert!(p.variant("v1").is_some());
        assert!(p.variant("missing").is_none());
    }

    #[test]
    fn variant_rejects_negative_price() {
        let mut v = variant_ok();
        v.price = Money(-1);
        assert!(v.validate().is_err());
    }

    #[test]
    fn variant_rejects_empty_sku() {
        let mut v = variant_ok();
        v.sku = "".into();
        assert!(v.validate().is_err());
    }

    #[test]
    fn variant_stock_check_finite_inventory() {
        let v = variant_ok();
        assert!(v.has_stock_for(1));
        assert!(v.has_stock_for(10));
        assert!(!v.has_stock_for(11));
    }

    #[test]
    fn variant_stock_check_unlimited_inventory() {
        let mut v = variant_ok();
        v.inventory_on_hand = None;
        assert!(v.has_stock_for(1));
        assert!(v.has_stock_for(u32::MAX));
    }

    #[test]
    fn reservation_terminal_set() {
        assert!(ReservationState::Committed.is_terminal());
        assert!(ReservationState::Released.is_terminal());
        assert!(!ReservationState::Held.is_terminal());
    }

    #[test]
    fn tax_jurisdiction_slugs_distinct() {
        let js = [
            TaxJurisdiction::EuVat,
            TaxJurisdiction::UkVat,
            TaxJurisdiction::UsSalesTax,
            TaxJurisdiction::CaGst,
            TaxJurisdiction::AuGst,
            TaxJurisdiction::None,
        ];
        let mut s = std::collections::HashSet::new();
        for j in js {
            assert!(s.insert(j.slug()));
        }
    }

    #[test]
    fn shipping_zone_covers_country() {
        let z = ShippingZone {
            id: "eu".into(),
            name: "EU".into(),
            countries: vec!["DE".into(), "FR".into(), "ES".into()],
            base_rate: Money(500),
            currency: usd(),
        };
        assert!(z.covers("DE"));
        assert!(z.covers("FR"));
        assert!(!z.covers("US"));
        assert!(!z.covers("de")); // case-sensitive — ISO 3166 alpha-2 is uppercase
    }

    #[test]
    fn subscription_cadence_approx_days_monotonic() {
        assert!(
            SubscriptionCadence::Weekly.approx_days() < SubscriptionCadence::Monthly.approx_days()
        );
        assert!(
            SubscriptionCadence::Monthly.approx_days()
                < SubscriptionCadence::Quarterly.approx_days()
        );
        assert!(
            SubscriptionCadence::Quarterly.approx_days()
                < SubscriptionCadence::Annual.approx_days()
        );
    }

    #[test]
    fn product_serde_round_trip() {
        let p = product_ok();
        let j = serde_json::to_string(&p).unwrap();
        let back: Product = serde_json::from_str(&j).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn product_rejects_unknown_field() {
        let bad =
            r#"{"id":"x","title":"t","description":"d","variants":[],"published":false,"ahem":1}"#;
        let r: Result<Product, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn currency_transparent_serde() {
        let usd = CurrencyCode::new("USD").unwrap();
        let j = serde_json::to_string(&usd).unwrap();
        assert_eq!(j, "\"USD\"");
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            ReservationState::Held,
            ReservationState::Committed,
            ReservationState::Released,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            TaxJurisdiction::EuVat,
            TaxJurisdiction::UkVat,
            TaxJurisdiction::UsSalesTax,
            TaxJurisdiction::CaGst,
            TaxJurisdiction::AuGst,
            TaxJurisdiction::None,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SubscriptionCadence::Weekly,
            SubscriptionCadence::Monthly,
            SubscriptionCadence::Quarterly,
            SubscriptionCadence::Annual,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
