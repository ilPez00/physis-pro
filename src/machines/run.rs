//! Serverless machine dispatch: run any reducer from a JSON value and get back
//! the same shape as the HTTP `POST /api/v1/machines/<name>` endpoints — but with
//! no server, no embedder, and no disk state (the reducers are pure). Used by the
//! `physis-pro machine <name>` CLI subcommand and available to lib embedders.
//!
//! Input/output mirror the web handlers field-for-field so the CLI and the HTTP
//! surface agree. (The web layer keeps its own typed handlers; if they ever drift,
//! this is the canonical reducer glue.)

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    apodeixis, chroma, chronos, elenchus, empedocles, erosa, erosp, euclid, harmony, kinesis,
    krisis, lex, pathos, plutos, polis, soma, techne, topos,
};

/// The list of machine names this dispatcher knows.
pub const NAMES: &[&str] = &[
    "chronos",
    "topos",
    "harmony",
    "polis",
    "soma",
    "elenchus",
    "lex",
    "empedocles",
    "euclid",
    "techne",
    "erosp",
    "erosa",
    "plutos",
    "krisis",
    "pathos",
    "chroma",
    "kinesis",
    "apodeixis",
];

/// A ready-to-edit example input for machine `name` (QA 8.7): the request
/// schemas were undiscoverable — you had to read this file to know what JSON
/// to feed `physis-pro machine <name>`. Every returned string is valid input
/// that actually reduces (enforced by [`tests::every_schema_example_runs`]),
/// so `physis-pro machine <name> --schema | physis-pro machine <name>` always
/// works.
pub fn schema(name: &str) -> Option<&'static str> {
    Some(match name {
        "chronos" => r#"{"events":[{"label":"oil check","at":"2026-08-01T08:00:00Z"},{"label":"oil check","at":"2026-08-08T08:05:00Z"},{"label":"oil check","at":"2026-08-15T07:55:00Z"}],"horizon_days":7,"tolerance":0.5}"#,
        "topos" => r#"{"waypoints":[{"lat":45.4642,"lng":9.1900,"label":"plant","at":"2026-08-20T09:00:00Z"},{"lat":45.4643,"lng":9.1901},{"lat":45.4700,"lng":9.2100}],"radius_km":0.5,"min_visits":2,"stale_days":30}"#,
        "harmony" => r#"{"notes":[60,64,67],"motif_len":4}"#,
        "polis" => r#"{"interactions":[{"from":"anna","to":"bruno","weight":1.0}],"min_strength":0.5}"#,
        "soma" => r#"{"sessions":[{"day":1,"activity":"run","load":5.0,"recovery":2.0}],"window":7}"#,
        "elenchus" => r#"{"statements":[{"id":"c1","kind":"claim","text":"the line output is stable"}]}"#,
        "lex" => r#"{"clauses":[{"id":"o1","kind":"obligation","text":"payment net 30","requires":[]}],"failed":[]}"#,
        "empedocles" => r#"{"readings":[{"metric":"temperature_c","value":61.0,"baseline":60.0}],"threshold":5.0,"series":[59.0,60.0,61.0]}"#,
        "euclid" => r#"{"primitives":[{"vertices":4,"circularity":0.9,"aspect_ratio":1.0,"color":[200,200,200],"texture":0.1,"bbox":[0.0,0.0,10.0,10.0]}],"circularity_limit":0.95,"near":2.0}"#,
        "techne" => r#"{"files":[{"path":"src/main.rs","lines":["fn main() {}"]}],"impact_target":"src/main.rs"}"#,
        "erosp" => r#"{"reaches":[{"subject":"team-a","object":"team-b","weight":0.8}],"min_intensity":0.5,"min_ascent":0.1}"#,
        "erosa" => r#"{"reaches":[{"subject":"team-a","object":"team-b","weight":0.8}],"min_balance":0.5}"#,
        "plutos" => r#"{"transactions":[{"from":"supplier","to":"plant","amount":100.0}],"min_count":2,"min_total":50.0}"#,
        "krisis" => r#"{"choices":[{"over":"ship now","under":"delay for more tests"}]}"#,
        "pathos" => r#"{"episodes":[{"trigger":"incident","valence":-0.8,"arousal":0.6}],"min_count":2,"flat":0.1}"#,
        "chroma" => r#"{"colors":[{"r":255,"g":0,"b":0}],"min_chroma":0.3,"tol":0.2}"#,
        "kinesis" => r#"{"gestures":[{"label":"swipe","points":[{"x":0.0,"y":0.0},{"x":10.0,"y":1.0}]}],"min_move":1.0,"min_net":0.5,"drop":0.8}"#,
        "apodeixis" => r#"{"obligations":[{"id":"chk-ls","claim":"ls exits cleanly","snippet":"true"}],"timeout_secs":30}"#,
        _ => return None,
    })
}

/// Run machine `name` over `input`, returning the reduced JSON.
pub fn run(name: &str, input: &Value) -> Result<Value> {    let v = input.clone();
    let out = match name {
        "chronos" => {
            #[derive(Deserialize)]
            struct Ev {
                label: String,
                at: String,
            }
            #[derive(Deserialize)]
            struct Req {
                events: Vec<Ev>,
                #[serde(default)]
                horizon_days: Option<i64>,
                #[serde(default)]
                tolerance: Option<f64>,
            }
            let r: Req = serde_json::from_value(v)?;
            let events: Vec<chronos::TimedEvent> = r
                .events
                .iter()
                .filter_map(|e| {
                    chrono::DateTime::parse_from_rfc3339(&e.at)
                        .ok()
                        .map(|dt| chronos::TimedEvent {
                            label: e.label.clone(),
                            at: dt.with_timezone(&chrono::Utc),
                        })
                })
                .collect();
            let rhythms = chronos::detect_rhythms(&events);
            let now = chrono::Utc::now();
            let horizon = r.horizon_days.unwrap_or(7);
            let tol = r.tolerance.unwrap_or(0.5);
            let upcoming: Vec<String> = chronos::upcoming(&rhythms, now, horizon)
                .iter()
                .map(|x| x.label.clone())
                .collect();
            let slipping: Vec<String> = chronos::slipping(&rhythms, now, tol)
                .iter()
                .map(|x| x.label.clone())
                .collect();
            let rj: Vec<Value> = rhythms.iter().map(|x| json!({ "label": x.label, "period_days": x.period_days, "count": x.count, "next_due": x.next_due.to_rfc3339(), "confidence": x.confidence })).collect();
            json!({ "rhythms": rj, "upcoming": upcoming, "slipping": slipping })
        }
        "topos" => {
            #[derive(Deserialize)]
            struct Wp {
                lat: f64,
                lng: f64,
                #[serde(default)]
                label: Option<String>,
                #[serde(default)]
                at: Option<String>,
            }
            #[derive(Deserialize)]
            struct Req {
                waypoints: Vec<Wp>,
                #[serde(default)]
                radius_km: Option<f64>,
                #[serde(default)]
                min_visits: Option<usize>,
                #[serde(default)]
                stale_days: Option<i64>,
            }
            let r: Req = serde_json::from_value(v)?;
            let wps: Vec<topos::Waypoint> = r
                .waypoints
                .iter()
                .map(|w| topos::Waypoint {
                    lat: w.lat,
                    lng: w.lng,
                    label: w.label.clone(),
                    at: w.at.as_ref().and_then(|s| {
                        chrono::DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    }),
                })
                .collect();
            let places =
                topos::detect_places(&wps, r.radius_km.unwrap_or(0.5), r.min_visits.unwrap_or(2));
            let now = chrono::Utc::now();
            let pj = |p: &topos::Place| json!({ "lat": p.lat, "lng": p.lng, "label": p.label, "visits": p.visits, "last": p.last.map(|d| d.to_rfc3339()), "radius_km": p.radius_km });
            let all: Vec<Value> = places.iter().map(pj).collect();
            let fading: Vec<Value> = topos::fading(&places, now, r.stale_days.unwrap_or(30))
                .into_iter()
                .map(pj)
                .collect();
            json!({ "places": all, "fading": fading })
        }
        "harmony" => {
            #[derive(Deserialize)]
            struct Req {
                notes: Vec<i32>,
                #[serde(default)]
                motif_len: Option<usize>,
            }
            let r: Req = serde_json::from_value(v)?;
            let intervals = harmony::intervals(&r.notes);
            let motifs: Vec<Value> = harmony::motifs(&r.notes, r.motif_len.unwrap_or(3))
                .into_iter()
                .map(|(m, c)| json!({ "intervals": m, "count": c }))
                .collect();
            json!({ "intervals": intervals, "motifs": motifs })
        }
        "polis" => {
            #[derive(Deserialize)]
            struct I {
                from: String,
                to: String,
                weight: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                interactions: Vec<I>,
                #[serde(default)]
                min_strength: Option<f32>,
                #[serde(default)]
                min_reciprocity: Option<f32>,
                #[serde(default)]
                max_reciprocity: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let ix: Vec<polis::Interaction> = r
                .interactions
                .iter()
                .map(|i| polis::Interaction {
                    from: i.from.clone(),
                    to: i.to.clone(),
                    weight: i.weight,
                })
                .collect();
            let ties = polis::ties(&ix);
            let tj = |t: &polis::Tie| json!({ "a": t.a, "b": t.b, "strength": t.strength, "reciprocity": t.reciprocity });
            json!({
                "ties": ties.iter().map(tj).collect::<Vec<_>>(),
                "strengthening": polis::strengthening(&ties, r.min_strength.unwrap_or(0.0), r.min_reciprocity.unwrap_or(0.5)).into_iter().map(tj).collect::<Vec<_>>(),
                "fading": polis::fading(&ties, r.max_reciprocity.unwrap_or(0.5)).into_iter().map(tj).collect::<Vec<_>>(),
            })
        }
        "soma" => {
            #[derive(Deserialize)]
            struct S {
                day: u32,
                activity: String,
                load: f32,
                recovery: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                sessions: Vec<S>,
                #[serde(default)]
                window: Option<usize>,
            }
            let r: Req = serde_json::from_value(v)?;
            let sessions: Vec<soma::Session> = r
                .sessions
                .iter()
                .map(|s| soma::Session {
                    day: s.day,
                    activity: s.activity.clone(),
                    load: s.load,
                    recovery: s.recovery,
                })
                .collect();
            let classified: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    let label = match soma::classify(s) {
                        soma::Status::Effort => "effort",
                        soma::Status::Recovery => "recovery",
                        soma::Status::Overreaching => "overreaching",
                    };
                    json!({ "day": s.day, "activity": s.activity, "status": label })
                })
                .collect();
            json!({ "sessions": classified, "drift": soma::drift(&sessions, r.window.unwrap_or(7)) })
        }
        "elenchus" => {
            #[derive(Deserialize)]
            struct St {
                id: String,
                kind: String,
                #[serde(default)]
                text: String,
                #[serde(default)]
                target: Option<String>,
            }
            #[derive(Deserialize)]
            struct Req {
                statements: Vec<St>,
            }
            let r: Req = serde_json::from_value(v)?;
            let statements: Vec<elenchus::Statement> = r
                .statements
                .iter()
                .filter_map(|s| {
                    let kind = match s.kind.to_ascii_lowercase().as_str() {
                        "claim" => elenchus::Kind::Claim,
                        "warrant" => elenchus::Kind::Warrant,
                        "rebuttal" => elenchus::Kind::Rebuttal,
                        _ => return None,
                    };
                    Some(elenchus::Statement {
                        id: s.id.clone(),
                        kind,
                        text: s.text.clone(),
                        target: s.target.clone(),
                    })
                })
                .collect();
            let standings: Vec<Value> = elenchus::evaluate(&statements)
                .into_iter()
                .map(|(id, st)| {
                    let label = match st {
                        elenchus::Standing::Supported => "supported",
                        elenchus::Standing::Contested => "contested",
                        elenchus::Standing::Unsupported => "unsupported",
                    };
                    json!({ "claim": id, "standing": label })
                })
                .collect();
            json!({ "standings": standings, "unsupported": elenchus::unsupported_claims(&statements), "contested": elenchus::contested_claims(&statements) })
        }
        "lex" => {
            #[derive(Deserialize)]
            struct C {
                id: String,
                kind: String,
                #[serde(default)]
                text: String,
                #[serde(default)]
                requires: Vec<String>,
            }
            #[derive(Deserialize)]
            struct Req {
                clauses: Vec<C>,
                #[serde(default)]
                failed: Vec<String>,
            }
            let r: Req = serde_json::from_value(v)?;
            let clauses: Vec<lex::Clause> = r
                .clauses
                .iter()
                .filter_map(|c| {
                    let kind = match c.kind.to_ascii_lowercase().as_str() {
                        "obligation" => lex::Kind::Obligation,
                        "right" => lex::Kind::Right,
                        "condition" => lex::Kind::Condition,
                        _ => return None,
                    };
                    Some(lex::Clause {
                        id: c.id.clone(),
                        kind,
                        text: c.text.clone(),
                        requires: c.requires.clone(),
                    })
                })
                .collect();
            json!({
                "unconditional_obligations": lex::unconditional_obligations(&clauses),
                "breach_paths": lex::breach_paths(&clauses, &r.failed),
                "dangling_conditions": lex::dangling_conditions(&clauses),
            })
        }
        "empedocles" => {
            #[derive(Deserialize)]
            struct Rd {
                metric: String,
                value: f32,
                baseline: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                readings: Vec<Rd>,
                #[serde(default)]
                threshold: Option<f32>,
                #[serde(default)]
                series: Option<Vec<f32>>,
            }
            let r: Req = serde_json::from_value(v)?;
            let threshold = r.threshold.unwrap_or(1.0);
            let readings: Vec<Value> = r.readings.iter().map(|rd| {
                let reading = empedocles::Reading { metric: rd.metric.clone(), value: rd.value, baseline: rd.baseline };
                let element = match reading.element() { empedocles::Element::Earth => "earth", empedocles::Element::Air => "air", empedocles::Element::Fire => "fire", empedocles::Element::Water => "water" };
                let status = match empedocles::classify(&reading, threshold) { empedocles::Status::Stable => "stable", empedocles::Status::Anomaly => "anomaly" };
                json!({ "metric": rd.metric, "element": element, "deviation": reading.deviation(), "status": status })
            }).collect();
            json!({ "readings": readings, "trend": r.series.as_ref().map(|s| empedocles::trend(s)) })
        }
        "euclid" => {
            #[derive(Deserialize)]
            struct P {
                vertices: u32,
                circularity: f32,
                #[serde(default)]
                aspect_ratio: f32,
                #[serde(default)]
                color: [u8; 3],
                #[serde(default)]
                texture: f32,
                #[serde(default)]
                bbox: [f32; 4],
            }
            #[derive(Deserialize)]
            struct Req {
                primitives: Vec<P>,
                #[serde(default)]
                circularity_limit: Option<f32>,
                #[serde(default)]
                near: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let limit = r.circularity_limit.unwrap_or(0.85);
            let near = r.near.unwrap_or(50.0);
            let prims: Vec<euclid::Primitive> = r
                .primitives
                .iter()
                .map(|p| euclid::Primitive {
                    vertices: p.vertices,
                    circularity: p.circularity,
                    aspect_ratio: p.aspect_ratio,
                    color: (p.color[0], p.color[1], p.color[2]),
                    texture: p.texture,
                    bbox: (p.bbox[0], p.bbox[1], p.bbox[2], p.bbox[3]),
                })
                .collect();
            let reduced: Vec<Value> = prims.iter().map(|p| {
                let st = euclid::classify(p, limit);
                json!({ "shape": euclid::classify_by_vertices(p.vertices), "status": if matches!(st, euclid::Status::Form) { "form" } else { "noise" }, "coherence": st.coherence(), "hash": euclid::compute_hash(p) })
            }).collect();
            let mut relations = Vec::new();
            for i in 0..prims.len() {
                for j in (i + 1)..prims.len() {
                    let name = match euclid::relation(&prims[i], &prims[j], near) {
                        euclid::Relation::Contained => "contained",
                        euclid::Relation::Near => "near",
                        euclid::Relation::Disjoint => continue,
                    };
                    relations.push(json!({ "a": i, "b": j, "relation": name }));
                }
            }
            json!({ "primitives": reduced, "relations": relations })
        }
        "techne" => {
            #[derive(Deserialize)]
            struct F {
                path: String,
                #[serde(default)]
                lines: Vec<String>,
            }
            #[derive(Deserialize)]
            struct Req {
                files: Vec<F>,
                #[serde(default)]
                impact_target: Option<String>,
            }
            let r: Req = serde_json::from_value(v)?;
            let files: Vec<techne::SourceFile> = r
                .files
                .iter()
                .map(|f| techne::SourceFile {
                    path: f.path.clone(),
                    lines: f.lines.clone(),
                })
                .collect();
            let caps: Vec<Value> = techne::classify_capabilities(&files)
                .iter()
                .map(|c| json!({ "name": c.name, "files": c.files }))
                .collect();
            let edges = techne::dependency_edges(&files);
            let deps: Vec<Value> = edges.iter().map(|(a, b)| json!([a, b])).collect();
            json!({ "capabilities": caps, "dependencies": deps, "impact": r.impact_target.as_ref().map(|t| techne::impact_of(&edges, t)) })
        }
        "erosp" => {
            #[derive(Deserialize)]
            struct Rch {
                subject: String,
                object: String,
                weight: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                reaches: Vec<Rch>,
                #[serde(default)]
                min_intensity: Option<f32>,
                #[serde(default)]
                min_ascent: Option<f32>,
                #[serde(default)]
                max_ascent: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let reaches: Vec<erosp::Reach> = r
                .reaches
                .iter()
                .map(|x| erosp::Reach {
                    subject: x.subject.clone(),
                    object: x.object.clone(),
                    weight: x.weight,
                })
                .collect();
            let longings = erosp::detect_longings(&reaches);
            let lj = |l: &erosp::Longing| json!({ "subject": l.subject, "object": l.object, "penia": l.penia, "poros": l.poros, "intensity": l.intensity, "ascent": l.ascent });
            json!({
                "longings": longings.iter().map(lj).collect::<Vec<_>>(),
                "ascending": erosp::ascending(&longings, r.min_intensity.unwrap_or(0.0), r.min_ascent.unwrap_or(0.1)).into_iter().map(lj).collect::<Vec<_>>(),
                "hopeless": erosp::hopeless(&longings, r.max_ascent.unwrap_or(0.05)).into_iter().map(lj).collect::<Vec<_>>(),
            })
        }
        "erosa" => {
            #[derive(Deserialize)]
            struct Rch {
                subject: String,
                object: String,
                weight: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                reaches: Vec<Rch>,
                #[serde(default)]
                min_balance: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let reaches: Vec<erosp::Reach> = r
                .reaches
                .iter()
                .map(|x| erosp::Reach {
                    subject: x.subject.clone(),
                    object: x.object.clone(),
                    weight: x.weight,
                })
                .collect();
            let conquests = erosa::detect_conquests(&reaches);
            let cj = |c: &erosa::Conquest| json!({ "subject": c.subject, "object": c.object, "aphrodite": c.aphrodite, "ares": c.ares, "rivals": c.rivals, "balance": c.balance });
            json!({
                "conquests": conquests.iter().map(cj).collect::<Vec<_>>(),
                "passionate": erosa::passionate(&conquests, r.min_balance.unwrap_or(0.5)).into_iter().map(cj).collect::<Vec<_>>(),
                "jealous": erosa::jealous(&conquests).into_iter().map(cj).collect::<Vec<_>>(),
            })
        }
        "plutos" => {
            #[derive(Deserialize)]
            struct T {
                from: String,
                to: String,
                amount: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                transactions: Vec<T>,
                #[serde(default)]
                min_count: Option<usize>,
                #[serde(default)]
                min_total: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let txns: Vec<plutos::Transaction> = r
                .transactions
                .iter()
                .map(|t| plutos::Transaction {
                    from: t.from.clone(),
                    to: t.to.clone(),
                    amount: t.amount,
                })
                .collect();
            let flows = plutos::detect_flows(&txns, r.min_count.unwrap_or(2));
            let bals = plutos::balances(&flows);
            let fj = |f: &plutos::Flow| json!({ "from": f.from, "to": f.to, "total": f.total, "count": f.count, "mean": f.mean });
            let bj = |b: &plutos::Balance| json!({ "party": b.party, "inflow": b.inflow, "outflow": b.outflow, "net": b.net });
            json!({
                "flows": flows.iter().map(fj).collect::<Vec<_>>(),
                "drains": plutos::drains(&flows, r.min_total.unwrap_or(0.0)).into_iter().map(fj).collect::<Vec<_>>(),
                "balances": bals.iter().map(bj).collect::<Vec<_>>(),
                "breaches": plutos::breaches(&bals).into_iter().map(bj).collect::<Vec<_>>(),
            })
        }
        "krisis" => {
            #[derive(Deserialize)]
            struct Ch {
                over: String,
                under: String,
            }
            #[derive(Deserialize)]
            struct Req {
                choices: Vec<Ch>,
            }
            let r: Req = serde_json::from_value(v)?;
            let choices: Vec<krisis::Choice> = r
                .choices
                .iter()
                .map(|c| krisis::Choice {
                    over: c.over.clone(),
                    under: c.under.clone(),
                })
                .collect();
            let prefs = krisis::detect_preferences(&choices);
            json!({
                "preferences": prefs.iter().map(|p| json!({ "over": p.over, "under": p.under, "margin": p.margin })).collect::<Vec<_>>(),
                "ranking": krisis::ranking(&prefs).iter().map(|x| json!({ "option": x.option, "score": x.score })).collect::<Vec<_>>(),
                "cycles": krisis::cycles(&prefs).into_iter().map(|c| json!(c)).collect::<Vec<_>>(),
            })
        }
        "pathos" => {
            #[derive(Deserialize)]
            struct Ep {
                trigger: String,
                valence: f32,
                arousal: f32,
            }
            #[derive(Deserialize)]
            struct Req {
                episodes: Vec<Ep>,
                #[serde(default)]
                min_count: Option<usize>,
                #[serde(default)]
                flat: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let episodes: Vec<pathos::Episode> = r
                .episodes
                .iter()
                .map(|e| pathos::Episode {
                    trigger: e.trigger.clone(),
                    valence: e.valence,
                    arousal: e.arousal,
                })
                .collect();
            let affects =
                pathos::detect_affects(&episodes, r.min_count.unwrap_or(2), r.flat.unwrap_or(0.1));
            let aj = |a: &pathos::Affect| json!({ "trigger": a.trigger, "valence": a.valence, "arousal": a.arousal, "count": a.count });
            json!({
                "affects": affects.iter().map(aj).collect::<Vec<_>>(),
                "amplifiers": pathos::amplifiers(&affects, 0.5).into_iter().map(aj).collect::<Vec<_>>(),
                "soothers": pathos::soothers(&affects, 0.5).into_iter().map(aj).collect::<Vec<_>>(),
            })
        }
        "chroma" => {
            #[derive(Deserialize)]
            struct Co {
                r: u8,
                g: u8,
                b: u8,
            }
            #[derive(Deserialize)]
            struct Req {
                colors: Vec<Co>,
                #[serde(default)]
                min_chroma: Option<f32>,
                #[serde(default)]
                tol: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let colors: Vec<chroma::Color> = r
                .colors
                .iter()
                .map(|c| chroma::Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                })
                .collect();
            let rels = chroma::detect_relations(
                &colors,
                r.min_chroma.unwrap_or(0.2),
                r.tol.unwrap_or(20.0),
            );
            let rj = |x: &chroma::Relation| {
                let kind = match x.kind {
                    chroma::Kind::Complement => "complement",
                    chroma::Kind::Analogous => "analogous",
                    chroma::Kind::Triad => "triad",
                    chroma::Kind::Clash => "clash",
                };
                json!({ "a": x.a, "b": x.b, "kind": kind })
            };
            json!({
                "relations": rels.iter().map(rj).collect::<Vec<_>>(),
                "harmonies": chroma::harmonies(&rels).into_iter().map(rj).collect::<Vec<_>>(),
                "clashes": chroma::clashes(&rels).into_iter().map(rj).collect::<Vec<_>>(),
            })
        }
        "kinesis" => {
            #[derive(Deserialize)]
            struct Pt {
                x: f32,
                y: f32,
            }
            #[derive(Deserialize)]
            struct G {
                label: String,
                points: Vec<Pt>,
            }
            #[derive(Deserialize)]
            struct Req {
                gestures: Vec<G>,
                #[serde(default)]
                min_move: Option<f32>,
                #[serde(default)]
                min_net: Option<f32>,
                #[serde(default)]
                drop: Option<f32>,
            }
            let r: Req = serde_json::from_value(v)?;
            let gestures: Vec<kinesis::Gesture> = r
                .gestures
                .iter()
                .map(|g| kinesis::Gesture {
                    label: g.label.clone(),
                    points: g
                        .points
                        .iter()
                        .map(|p| kinesis::Point { x: p.x, y: p.y })
                        .collect(),
                })
                .collect();
            let motions = kinesis::detect_motions(&gestures, r.min_move.unwrap_or(0.5));
            let mj = |m: &kinesis::Motion| json!({ "label": m.label, "path": m.path, "net": m.net, "straightness": m.straightness, "moves": m.moves });
            json!({
                "motions": motions.iter().map(mj).collect::<Vec<_>>(),
                "reaches": kinesis::reaches(&motions, r.min_net.unwrap_or(1.0)).into_iter().map(mj).collect::<Vec<_>>(),
                "degrading": kinesis::degrading(&motions, r.drop.unwrap_or(0.2)),
            })
        }
        "apodeixis" => {
            #[derive(Deserialize)]
            struct Ob {
                id: String,
                #[serde(default)]
                claim: String,
                snippet: String,
            }
            #[derive(Deserialize)]
            struct Req {
                obligations: Vec<Ob>,
                #[serde(default)]
                timeout_secs: Option<u64>,
            }
            let r: Req = serde_json::from_value(v)?;
            let obligations: Vec<apodeixis::Obligation> = r
                .obligations
                .iter()
                .map(|o| apodeixis::Obligation {
                    id: o.id.clone(),
                    claim: o.claim.clone(),
                    snippet: o.snippet.clone(),
                })
                .collect();
            let checks = apodeixis::reduce(&obligations, r.timeout_secs.unwrap_or(30));
            let cj: Vec<Value> = checks
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id, "claim": c.claim, "verdict": c.verdict.as_str(),
                        "witness": c.witness, "exit_code": c.exit_code, "engine": c.engine
                    })
                })
                .collect();
            let rep = apodeixis::evaluate(&checks);
            json!({ "checks": cj, "certified": rep.certified, "refuted": rep.refuted, "untestable": rep.untestable })
        }
        other => bail!("unknown machine '{other}'. known: {}", NAMES.join(", ")),
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression (QA 8.7): every machine must publish a schema example, and
    /// every example must be executable input — not documentation that rots.
    #[test]
    fn every_schema_example_runs() {
        for name in NAMES {
            let ex = schema(name).unwrap_or_else(|| panic!("{name} has no schema example"));
            let v: Value = serde_json::from_str(ex)
                .unwrap_or_else(|e| panic!("{name} schema is not valid JSON: {e}"));
            run(name, &v).unwrap_or_else(|e| panic!("{name} schema does not reduce: {e}"));
        }
        assert!(schema("nope").is_none());
    }

    #[test]
    fn dispatch_runs_a_few_machines() {
        let out = run(
            "plutos",
            &json!({ "transactions": [
            {"from":"me","to":"rent","amount":1000.0},{"from":"me","to":"rent","amount":1000.0}
        ], "min_count": 2 }),
        )
        .unwrap();
        assert!(out["flows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["to"] == "rent"));

        let out = run(
            "krisis",
            &json!({ "choices": [
            {"over":"a","under":"b"},{"over":"b","under":"c"},{"over":"c","under":"a"}
        ] }),
        )
        .unwrap();
        assert_eq!(out["cycles"].as_array().unwrap().len(), 1);

        let out = run(
            "apodeixis",
            &json!({ "obligations": [
            {"id":"o1","claim":"unit","snippet":"#check True"}
        ], "timeout_secs": 5 }),
        )
        .unwrap();
        assert_eq!(out["checks"].as_array().unwrap().len(), 1);
        assert!(out["checks"][0]["verdict"].as_str().is_some());

        assert!(run("nope", &json!({})).is_err());
    }
}
