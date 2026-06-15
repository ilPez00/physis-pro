//! topos — the spatial machine. (Aristotle's *topos*, place; the method of loci.)
//!
//! Lived-space twin of `chronos`: chronos eats the *time* of events, topos eats
//! the *place*. Perceive geotagged waypoints; reduce to **place** primitives —
//! a recurring location (a clustered region visited ≥ N times), not a raw GPS
//! fix; elaborate by flagging places that have gone quiet (fading regions). One-
//! off coordinates and jitter are rejected (a place must be *elaborable* — like
//! euclid refusing the circle).
//!
//! Pure + testable: `detect_places` / `fading` take waypoints and return places;
//! `register_places` is the only part that touches the core. Source data already
//! exists in Praxis (notebook waypoints carry lat/lng + time).

use chrono::{DateTime, Duration, Utc};

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One observed location, optionally labelled and timestamped.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub lat: f64,
    pub lng: f64,
    pub label: Option<String>,
    pub at: Option<DateTime<Utc>>,
}

/// A recurring place: a cluster centroid with a visit count.
#[derive(Debug, Clone)]
pub struct Place {
    pub lat: f64,
    pub lng: f64,
    /// Most common non-empty label among the cluster's waypoints, if any.
    pub label: Option<String>,
    pub visits: usize,
    /// Most recent visit time, if any waypoint carried one.
    pub last: Option<DateTime<Utc>>,
    /// Cluster radius used to group points.
    pub radius_km: f64,
}

/// Great-circle distance in km between two lat/lng points.
pub fn haversine_km(a_lat: f64, a_lng: f64, b_lat: f64, b_lng: f64) -> f64 {
    const R: f64 = 6371.0;
    let (la1, la2) = (a_lat.to_radians(), b_lat.to_radians());
    let dla = (b_lat - a_lat).to_radians();
    let dlo = (b_lng - a_lng).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().asin()
}

struct Cluster {
    lat: f64,
    lng: f64,
    visits: usize,
    last: Option<DateTime<Utc>>,
    labels: std::collections::HashMap<String, usize>,
}

/// Reduce waypoints to recurring places. Greedy single-pass clustering: each
/// point joins the first cluster within `radius_km` (centroid updated as a running
/// mean), else starts a new one. Clusters with fewer than `min_visits` points are
/// rejected as noise. Returned most-visited first.
pub fn detect_places(waypoints: &[Waypoint], radius_km: f64, min_visits: usize) -> Vec<Place> {
    let mut clusters: Vec<Cluster> = Vec::new();
    for w in waypoints {
        let found = clusters
            .iter_mut()
            .find(|c| haversine_km(c.lat, c.lng, w.lat, w.lng) <= radius_km);
        match found {
            Some(c) => {
                let n = c.visits as f64;
                c.lat = (c.lat * n + w.lat) / (n + 1.0);
                c.lng = (c.lng * n + w.lng) / (n + 1.0);
                c.visits += 1;
                if let Some(at) = w.at {
                    c.last = Some(c.last.map_or(at, |l| l.max(at)));
                }
                if let Some(lbl) = &w.label {
                    if !lbl.is_empty() {
                        *c.labels.entry(lbl.clone()).or_default() += 1;
                    }
                }
            }
            None => clusters.push(Cluster {
                lat: w.lat,
                lng: w.lng,
                visits: 1,
                last: w.at,
                labels: w
                    .label
                    .as_ref()
                    .filter(|l| !l.is_empty())
                    .map(|l| std::iter::once((l.clone(), 1usize)).collect())
                    .unwrap_or_default(),
            }),
        }
    }

    let mut places: Vec<Place> = clusters
        .into_iter()
        .filter(|c| c.visits >= min_visits)
        .map(|c| {
            let label = c
                .labels
                .into_iter()
                .max_by_key(|(_, n)| *n)
                .map(|(l, _)| l);
            Place {
                lat: c.lat,
                lng: c.lng,
                label,
                visits: c.visits,
                last: c.last,
                radius_km,
            }
        })
        .collect();
    places.sort_by(|a, b| b.visits.cmp(&a.visits));
    places
}

/// Places not visited within `stale_days` of `now` — the fading regions a dream
/// would surface (a place slipping out of the life before it's lost).
pub fn fading(places: &[Place], now: DateTime<Utc>, stale_days: i64) -> Vec<&Place> {
    let cutoff = now - Duration::days(stale_days);
    places
        .iter()
        .filter(|p| matches!(p.last, Some(last) if last < cutoff))
        .collect()
}

/// Adapter: register each place as a labelled, bridged coherence node. Returns ids.
pub fn register_places(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    places: &[Place],
) -> Vec<String> {
    let mut ids = Vec::with_capacity(places.len());
    for p in places {
        let name = p.label.clone().unwrap_or_else(|| format!("{:.4},{:.4}", p.lat, p.lng));
        let text = format!("place: {} ({} visits)", name, p.visits);
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wp(lat: f64, lng: f64) -> Waypoint {
        Waypoint { lat, lng, label: None, at: None }
    }

    #[test]
    fn clusters_nearby_points_and_rejects_singletons() {
        // Three visits to ~the same spot, plus one far-away one-off.
        let wps = vec![
            wp(45.4642, 9.1900),
            wp(45.4643, 9.1901),
            wp(45.4641, 9.1899),
            wp(48.8566, 2.3522), // Paris — single visit, should be rejected
        ];
        let places = detect_places(&wps, 0.5, 2);
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].visits, 3);
    }

    #[test]
    fn fading_flags_stale_places() {
        let now = Utc::now();
        let old = now - Duration::days(40);
        let wps: Vec<Waypoint> = (0..3)
            .map(|_| Waypoint { lat: 45.0, lng: 9.0, label: Some("gym".into()), at: Some(old) })
            .collect();
        let places = detect_places(&wps, 0.5, 2);
        assert_eq!(places.len(), 1);
        assert_eq!(fading(&places, now, 30).len(), 1);
        assert_eq!(fading(&places, now, 60).len(), 0);
    }
}
