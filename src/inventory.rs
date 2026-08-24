//! Inventory tracking, demand forecasting, and human-in-loop ordering.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// An inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub reorder_point: i32,
    pub reorder_qty: i32,
    pub unit: String,
    pub lead_time_days: u32,
}

/// A historical consumption record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionRecord {
    pub date: chrono::NaiveDate,
    pub sku: String,
    pub quantity: i32,
}

/// An ordering suggestion (human must approve when order_mode = "manual").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSuggestion {
    pub sku: String,
    pub name: String,
    pub suggested_qty: i32,
    pub reason: String,
    pub forecast_daily_usage: f32,
    pub days_until_stockout: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Simple inventory tracker backed by in-memory state.
pub struct InventoryTracker {
    pub items: HashMap<String, InventoryItem>,
    pub consumption: Vec<ConsumptionRecord>,
    pub order_cooldown_hours: u64,
    pub last_order_at: HashMap<String, chrono::DateTime<chrono::Utc>>,
    /// Every suggestion actually recorded by an order, newest last — the
    /// audit trail `inventory` renders. QA 8.4: orders used to evaporate.
    pub order_log: Vec<OrderSuggestion>,
}

impl InventoryTracker {
    pub fn new(order_cooldown_hours: u64) -> Self {
        Self {
            items: HashMap::new(),
            consumption: Vec::new(),
            order_cooldown_hours,
            last_order_at: HashMap::new(),
            order_log: Vec::new(),
        }
    }

    /// Register or update an inventory item.
    pub fn upsert(&mut self, item: InventoryItem) {
        self.items.insert(item.sku.clone(), item);
    }

    /// Record consumption of an item.
    pub fn consume(&mut self, sku: &str, qty: i32) {
        self.consumption.push(ConsumptionRecord {
            date: chrono::Utc::now().date_naive(),
            sku: sku.to_string(),
            quantity: qty,
        });
        if let Some(item) = self.items.get_mut(sku) {
            item.quantity -= qty;
        }
    }

    /// Compute forecast using simple moving average of last N days.
    pub fn forecast_daily_usage(&self, sku: &str, window_days: i64) -> f32 {
        let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(window_days);
        let relevant: Vec<&ConsumptionRecord> = self
            .consumption
            .iter()
            .filter(|r| r.sku == sku && r.date >= cutoff)
            .collect();
        if relevant.is_empty() {
            return 0.0;
        }
        let total: i32 = relevant.iter().map(|r| r.quantity).sum();
        total as f32 / window_days as f32
    }

    /// Check if an item needs reordering. Returns None if not needed.
    pub fn check_reorder(&self, sku: &str) -> Option<OrderSuggestion> {
        let item = self.items.get(sku)?;
        if item.quantity > item.reorder_point {
            return None;
        }
        let daily = self.forecast_daily_usage(sku, 30);
        let days_until_stockout = if daily > 0.0 {
            (item.quantity as f32 / daily).ceil() as i32
        } else {
            999
        };
        Some(OrderSuggestion {
            sku: sku.to_string(),
            name: item.name.clone(),
            suggested_qty: item.reorder_qty,
            reason: format!(
                "qty {} below reorder point {}",
                item.quantity, item.reorder_point
            ),
            forecast_daily_usage: daily,
            days_until_stockout,
            created_at: chrono::Utc::now(),
        })
    }

    /// Check all items and return all needed order suggestions.
    pub fn check_all_reorders(&self) -> Vec<OrderSuggestion> {
        let mut suggestions = Vec::new();
        for sku in self.items.keys() {
            if let Some(suggestion) = self.check_reorder(sku) {
                if let Some(last) = self.last_order_at.get(sku) {
                    let elapsed = (chrono::Utc::now() - *last).num_hours() as u64;
                    if elapsed < self.order_cooldown_hours {
                        continue;
                    }
                }
                suggestions.push(suggestion);
            }
        }
        suggestions
    }

    /// Mark that an order was placed for a SKU.
    pub fn record_order(&mut self, sku: &str) {
        self.last_order_at
            .insert(sku.to_string(), chrono::Utc::now());
        if let Some(item) = self.items.get_mut(sku) {
            item.quantity += item.reorder_qty;
        }
    }

    /// Record a manual order (the human typed `physis-pro order <sku> <qty>`):
    /// stamp the cooldown clock and log the suggestion. The SKU is registered
    /// on first sight so later consumption/forecast flows know it. Ordering is
    /// not receiving — stock levels are untouched until consumption says so.
    ///
    /// `Err(remaining)` when the SKU is still inside the order cooldown.
    pub fn place_manual_order(
        &mut self,
        sku: &str,
        qty: i32,
    ) -> Result<OrderSuggestion, chrono::Duration> {
        if let Some(last) = self.last_order_at.get(sku) {
            let elapsed = chrono::Utc::now() - *last;
            let cooldown = chrono::Duration::hours(self.order_cooldown_hours as i64);
            if elapsed < cooldown {
                return Err(cooldown - elapsed);
            }
        }

        if !self.items.contains_key(sku) {
            self.upsert(InventoryItem {
                sku: sku.to_string(),
                name: sku.to_string(),
                quantity: 0,
                reorder_point: 0,
                reorder_qty: qty,
                unit: "pcs".to_string(),
                lead_time_days: 0,
            });
        }
        let name = self.items[sku].name.clone();
        let now = chrono::Utc::now();
        self.last_order_at.insert(sku.to_string(), now);
        let suggestion = OrderSuggestion {
            sku: sku.to_string(),
            name,
            suggested_qty: qty,
            reason: "manual order (human-in-loop)".to_string(),
            forecast_daily_usage: self.forecast_daily_usage(sku, 30),
            days_until_stockout: 0,
            created_at: now,
        };
        self.order_log.push(suggestion.clone());
        Ok(suggestion)
    }

    /// Save inventory to sled database (requires sled feature).
    pub fn save(&self, db: &sled::Db) -> anyhow::Result<()> {
        let json = serde_json::to_string(&self.items)?;
        db.insert("inventory_items", json.as_bytes())?;
        let cons_json = serde_json::to_string(&self.consumption)?;
        db.insert("consumption", cons_json.as_bytes())?;
        let orders_json = serde_json::to_string(&self.last_order_at)?;
        db.insert("last_order_at", orders_json.as_bytes())?;
        let log_json = serde_json::to_string(&self.order_log)?;
        db.insert("order_log", log_json.as_bytes())?;
        Ok(())
    }

    /// Load inventory from sled database.
    pub fn load(db: &sled::Db, order_cooldown_hours: u64) -> anyhow::Result<Self> {
        let items = match db.get("inventory_items")? {
            Some(data) => serde_json::from_slice(&data)?,
            None => HashMap::new(),
        };
        let consumption = match db.get("consumption")? {
            Some(data) => serde_json::from_slice(&data)?,
            None => Vec::new(),
        };
        let last_order_at = match db.get("last_order_at")? {
            Some(data) => serde_json::from_slice(&data)?,
            None => HashMap::new(),
        };
        let order_log = match db.get("order_log")? {
            Some(data) => serde_json::from_slice(&data)?,
            None => Vec::new(),
        };
        Ok(Self {
            items,
            consumption,
            order_cooldown_hours,
            last_order_at,
            order_log,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_tracker() -> InventoryTracker {
        let mut t = InventoryTracker::new(24);
        t.upsert(InventoryItem {
            sku: "BOLT-M10".into(),
            name: "M10 Bolt".into(),
            quantity: 50,
            reorder_point: 100,
            reorder_qty: 500,
            unit: "pcs".into(),
            lead_time_days: 3,
        });
        t.upsert(InventoryItem {
            sku: "NUT-M10".into(),
            name: "M10 Nut".into(),
            quantity: 500,
            reorder_point: 100,
            reorder_qty: 1000,
            unit: "pcs".into(),
            lead_time_days: 2,
        });
        t
    }

    #[test]
    fn test_upsert_and_get() {
        let t = setup_tracker();
        assert!(t.items.contains_key("BOLT-M10"));
        assert!(t.items.contains_key("NUT-M10"));
    }

    #[test]
    fn test_consume_reduces_quantity() {
        let mut t = setup_tracker();
        t.consume("BOLT-M10", 10);
        assert_eq!(t.items["BOLT-M10"].quantity, 40);
    }

    #[test]
    fn test_check_reorder_triggers_below_point() {
        let t = setup_tracker();
        let suggestion = t.check_reorder("BOLT-M10");
        assert!(
            suggestion.is_some(),
            "50 qty < 100 reorder point should trigger"
        );
        let s = suggestion.unwrap();
        assert_eq!(s.suggested_qty, 500);
    }

    #[test]
    fn test_check_reorder_does_not_trigger_above_point() {
        let t = setup_tracker();
        let suggestion = t.check_reorder("NUT-M10");
        assert!(
            suggestion.is_none(),
            "500 qty >= 100 reorder point should not trigger"
        );
    }

    #[test]
    fn test_check_all_reorders() {
        let t = setup_tracker();
        let suggestions = t.check_all_reorders();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].sku, "BOLT-M10");
    }

    #[test]
    fn test_forecast_empty() {
        let t = setup_tracker();
        let daily = t.forecast_daily_usage("BOLT-M10", 30);
        assert!((daily - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_forecast_with_consumption() {
        let mut t = setup_tracker();
        t.consume("BOLT-M10", 60);
        t.consume("BOLT-M10", 30);
        let daily = t.forecast_daily_usage("BOLT-M10", 30);
        assert!((daily - 3.0).abs() < 0.01, "90 units / 30 days = 3.0");
    }

    #[test]
    fn test_record_order_increases_quantity() {
        let mut t = setup_tracker();
        t.record_order("BOLT-M10");
        assert_eq!(t.items["BOLT-M10"].quantity, 550);
    }

    #[test]
    fn test_cooldown_suppresses_duplicate() {
        let mut t = setup_tracker();
        t.record_order("BOLT-M10");
        // Immediately after record_order, cooldown should suppress
        let suggestions = t.check_all_reorders();
        assert!(suggestions.is_empty(), "cooldown should suppress reorder");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let mut t = setup_tracker();
        t.consume("BOLT-M10", 5);
        let db = sled::Config::new().temporary(true).open().unwrap();
        t.save(&db).unwrap();

        let loaded = InventoryTracker::load(&db, 24).unwrap();
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items["BOLT-M10"].quantity, 45);
    }

    /// Regression (QA 8.4): a manual order must be logged, stamped into the
    /// cooldown, and survive a save/load roundtrip — not just print a line.
    #[test]
    fn manual_order_is_logged_and_persisted() {
        let mut t = InventoryTracker::new(24);
        let s = t.place_manual_order("GREASE-400G", 10).unwrap();
        assert_eq!(s.sku, "GREASE-400G");
        assert_eq!(s.suggested_qty, 10);

        // The SKU was unknown before — it is now registered for future flows.
        assert!(t.items.contains_key("GREASE-400G"));
        // Ordering is not receiving: stock stays at its registered zero.
        assert_eq!(t.items["GREASE-400G"].quantity, 0);
        // Cooldown clock is running.
        assert!(t.last_order_at.contains_key("GREASE-400G"));

        let db = sled::Config::new().temporary(true).open().unwrap();
        t.save(&db).unwrap();
        let loaded = InventoryTracker::load(&db, 24).unwrap();
        assert_eq!(loaded.order_log.len(), 1);
        assert_eq!(loaded.order_log[0].sku, "GREASE-400G");
        assert!(loaded.last_order_at.contains_key("GREASE-400G"));
    }

    /// The cooldown applies to manual orders too; `inventory` shows the wait.
    #[test]
    fn manual_order_respects_cooldown() {
        let mut t = InventoryTracker::new(24);
        t.place_manual_order("BOLT-M10", 100).unwrap();
        let err = t.place_manual_order("BOLT-M10", 50).unwrap_err();
        assert!(
            err > chrono::Duration::zero(),
            "remaining cooldown reported"
        );
    }

    /// After the cooldown window has passed the same SKU can be ordered again.
    #[test]
    fn manual_order_allowed_after_cooldown() {
        let mut t = InventoryTracker::new(24);
        t.place_manual_order("BOLT-M10", 100).unwrap();
        // Age the stamp past the window.
        t.last_order_at.insert(
            "BOLT-M10".into(),
            chrono::Utc::now() - chrono::Duration::hours(25),
        );
        assert!(t.place_manual_order("BOLT-M10", 50).is_ok());
        assert_eq!(t.order_log.len(), 2);
    }
}
