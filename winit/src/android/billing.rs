//! Google Play Billing over the `taim.billing` extension (project.py `--billing`).
//!
//! Never grant entitlement from client data alone: send `Purchase::token` to your server,
//! verify it with the Google Play Developer API, then acknowledge or consume within 3 days
//! (otherwise Google refunds the purchase).
use serde::Deserialize;
use serde_json::Value;

/// Extension channel name, for filtering `PlatformEvent::Plugin`.
pub const BILLING_CHANNEL: &str = "taim.billing";

/// Product kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductKind {
    /// One-time product (consumable or not).
    Inapp,
    /// Subscription.
    Subs,
}
impl ProductKind {
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn wire(self) -> &'static str {
        match self {
            Self::Inapp => "inapp",
            Self::Subs => "subs",
        }
    }
}

/// A localized price.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Price {
    /// Amount in micro-units (1,000,000 = 1.00).
    pub micros: i64,
    /// Display string, e.g. "₹99.00".
    pub formatted: String,
    /// ISO 4217 code.
    pub currency: String,
}
/// One subscription pricing phase (trial, intro, base).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PricingPhase {
    /// Phase price.
    #[serde(flatten)]
    pub price: Price,
    /// ISO 8601 billing period, e.g. "P1M".
    pub period: String,
    /// Number of billing cycles (0 for infinite recurring).
    pub cycles: u32,
}
/// A subscription offer; pass its token to `billing_purchase`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionOffer {
    /// Offer token.
    pub token: String,
    /// Base plan id.
    pub base_plan_id: String,
    /// Offer id (None for the base plan itself).
    pub offer_id: Option<String>,
    /// Pricing phases in order.
    pub phases: Vec<PricingPhase>,
}
/// Product details from Play.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Product {
    /// Product id.
    pub id: String,
    /// Kind.
    #[serde(rename = "type")]
    pub kind: ProductKind,
    /// Title including the app name.
    pub title: String,
    /// Short name.
    pub name: String,
    /// Description.
    pub description: String,
    /// One-time price (in-app products).
    pub price: Option<Price>,
    /// Subscription offers.
    pub offers: Vec<SubscriptionOffer>,
}

/// Purchase state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PurchaseState {
    /// Paid; verify server-side, then acknowledge/consume.
    Purchased,
    /// Awaiting payment (e.g. cash); do not grant yet.
    Pending,
    /// Unknown.
    Unspecified,
}
/// A purchase record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    /// Google order id.
    pub order_id: Option<String>,
    /// Product ids.
    pub products: Vec<String>,
    /// Purchase token for server verification and acknowledgement.
    pub token: String,
    /// State.
    pub state: PurchaseState,
    /// Already acknowledged.
    pub acknowledged: bool,
    /// Subscription auto-renews.
    pub auto_renewing: bool,
    /// Purchase time, ms since the Unix epoch.
    pub time_ms: u64,
    /// Quantity.
    pub quantity: u32,
    /// Signed JSON (verify with your Play public key or, preferably, server-side).
    pub original_json: String,
    /// Signature of `original_json`.
    pub signature: String,
}

/// Outcome of a purchase flow, delivered as a plugin event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchaseUpdate {
    /// "ok", "cancelled", or an error code (unavailable, busy = already owned, ...).
    pub result: String,
    /// Purchases changed by the flow.
    pub purchases: Vec<Purchase>,
}
impl PurchaseUpdate {
    /// Decode a `PlatformEvent::Plugin { channel, event }`.
    pub fn decode(channel: &str, event: &Value) -> Option<Self> {
        if channel != BILLING_CHANNEL || event["type"] != "purchases" {
            return None;
        }
        Some(Self {
            result: event["result"].as_str()?.to_owned(),
            purchases: serde_json::from_value(event["purchases"].clone()).ok()?,
        })
    }
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::parse;
    use super::super::{ServiceError, Services};
    use super::*;
    use serde_json::json;
    const C: &str = BILLING_CHANNEL;
    impl Services {
        /// Connect to Play Billing (idempotent). Call again after `Unavailable`.
        pub async fn billing_connect(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "connect", json!({})).await.map(drop)
        }
        /// Query 1–100 products of one kind. Must precede `billing_purchase` for those ids.
        pub async fn billing_products(
            &self,
            kind: ProductKind,
            ids: &[&str],
        ) -> Result<Vec<Product>, ServiceError> {
            if ids.is_empty()
                || ids.len() > 100
                || ids.iter().any(|id| id.is_empty() || id.len() > 150)
            {
                return Err(ServiceError::Invalid);
            }
            parse(
                self.plugin_call(C, "products", json!({"type":kind.wire(),"ids":ids}))
                    .await?,
            )
            .ok_or(ServiceError::Failed)
        }
        /// Open the Play purchase sheet. Returns once it is shown; the outcome is a `PurchaseUpdate` event.
        /// Subscriptions require an `offer_token`. `account_id` should be an obfuscated (hashed) user id.
        pub async fn billing_purchase(
            &self,
            product_id: &str,
            offer_token: Option<&str>,
            account_id: Option<&str>,
        ) -> Result<(), ServiceError> {
            self.plugin_call(
                C,
                "purchase",
                json!({"productId":product_id,"offerToken":offer_token,"accountId":account_id}),
            )
            .await
            .map(drop)
        }
        /// Owned purchases (restore). Includes unacknowledged ones that still need processing.
        pub async fn billing_purchases(
            &self,
            kind: ProductKind,
        ) -> Result<Vec<Purchase>, ServiceError> {
            parse(
                self.plugin_call(C, "purchases", json!({"type":kind.wire()}))
                    .await?,
            )
            .ok_or(ServiceError::Failed)
        }
        /// Acknowledge a verified non-consumable purchase or subscription.
        pub async fn billing_acknowledge(&self, token: &str) -> Result<(), ServiceError> {
            self.plugin_call(C, "acknowledge", json!({"token":token}))
                .await
                .map(drop)
        }
        /// Consume a verified consumable so it can be bought again.
        pub async fn billing_consume(&self, token: &str) -> Result<(), ServiceError> {
            self.plugin_call(C, "consume", json!({"token":token}))
                .await
                .map(drop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn products_and_updates_decode() {
        let p: Product = serde_json::from_value(json!({"id":"pro","type":"subs","title":"Pro (App)","name":"Pro","description":"d","price":null,
            "offers":[{"token":"t","basePlanId":"monthly","offerId":null,"phases":[{"micros":99000000,"formatted":"₹99","currency":"INR","period":"P1M","cycles":0}]}]})).unwrap();
        assert_eq!(p.kind, ProductKind::Subs);
        assert_eq!(p.offers[0].phases[0].price.micros, 99_000_000);
        let u = json!({"type":"purchases","result":"cancelled","purchases":[]});
        assert_eq!(
            PurchaseUpdate::decode(BILLING_CHANNEL, &u).unwrap().result,
            "cancelled"
        );
        assert!(PurchaseUpdate::decode("x", &u).is_none());
    }
}
