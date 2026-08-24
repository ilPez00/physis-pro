use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::HeaderMap,
};

/// GET /api/v1/machines — registry of the philosophical machines.
pub async fn machines_list_handler() -> Json<serde_json::Value> {
    let machines = [
        MachineInfo {
            name: "chronos",
            modality: "timed events",
            primitive: "rhythm (recurrence)",
            status: "live",
        },
        MachineInfo {
            name: "topos",
            modality: "geotagged waypoints",
            primitive: "place / region (recurring)",
            status: "live",
        },
        MachineInfo {
            name: "harmony",
            modality: "notes (pitches)",
            primitive: "interval / motif",
            status: "live",
        },
        MachineInfo {
            name: "elenchus",
            modality: "dialogue statements",
            primitive: "claim / warrant / rebuttal",
            status: "live",
        },
        MachineInfo {
            name: "lex",
            modality: "contract clauses",
            primitive: "obligation / right / condition",
            status: "live",
        },
        MachineInfo {
            name: "soma",
            modality: "training sessions",
            primitive: "effort / recovery / drift",
            status: "live",
        },
        MachineInfo {
            name: "polis",
            modality: "social interactions",
            primitive: "affinity / reciprocity tie",
            status: "live",
        },
        MachineInfo {
            name: "empedocles",
            modality: "physical readings",
            primitive: "element (earth/air/fire/water)",
            status: "live",
        },
        MachineInfo {
            name: "erosp",
            modality: "directed reaches",
            primitive: "longing (Penia/Poros)",
            status: "live",
        },
        MachineInfo {
            name: "erosa",
            modality: "directed reaches",
            primitive: "conquest (Ares/Aphrodite)",
            status: "live",
        },
        MachineInfo {
            name: "techne",
            modality: "source files",
            primitive: "capability / dependency",
            status: "live",
        },
        MachineInfo {
            name: "euclid",
            modality: "visual primitives",
            primitive: "polygon (vertices)",
            status: "live",
        },
        MachineInfo {
            name: "plutos",
            modality: "transactions",
            primitive: "value flow (recurring)",
            status: "live",
        },
        MachineInfo {
            name: "krisis",
            modality: "pairwise choices",
            primitive: "preference (A > B)",
            status: "live",
        },
        MachineInfo {
            name: "pathos",
            modality: "affect episodes",
            primitive: "affect (valence x arousal)",
            status: "live",
        },
        MachineInfo {
            name: "chroma",
            modality: "colors",
            primitive: "color relation (wheel)",
            status: "live",
        },
        MachineInfo {
            name: "kinesis",
            modality: "trajectories",
            primitive: "motion gestalt (reach/gait)",
            status: "live",
        },
        MachineInfo {
            name: "apodeixis",
            modality: "lean proof obligations",
            primitive: "verdict (certified / refuted)",
            status: "live",
        },
    ];
    Json(serde_json::json!({ "machines": machines }))
}

/// POST /api/v1/machines/chronos — reduce timed events to recurrence rhythms,
/// and partition them into upcoming (due within horizon) and slipping (overdue).
/// Stateless: nothing is written to the core.
pub async fn machines_chronos_handler(Json(req): Json<ChronosRequest>) -> Json<serde_json::Value> {
    use crate::machines::chronos;

    let events: Vec<chronos::TimedEvent> = req
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
    let horizon = req.horizon_days.unwrap_or(7);
    let tol = req.tolerance.unwrap_or(0.5);

    let upcoming: Vec<String> = chronos::upcoming(&rhythms, now, horizon)
        .iter()
        .map(|r| r.label.clone())
        .collect();
    let slipping: Vec<String> = chronos::slipping(&rhythms, now, tol)
        .iter()
        .map(|r| r.label.clone())
        .collect();

    let rhythms_json: Vec<serde_json::Value> = rhythms
        .iter()
        .map(|r| {
            serde_json::json!({
                "label": r.label,
                "period_days": r.period_days,
                "count": r.count,
                "last": r.last.to_rfc3339(),
                "next_due": r.next_due.to_rfc3339(),
                "confidence": r.confidence,
            })
        })
        .collect();

    Json(serde_json::json!({
        "rhythms": rhythms_json,
        "upcoming": upcoming,
        "slipping": slipping,
    }))
}

/// POST /api/v1/machines/harmony — reduce a melody to key-invariant intervals
/// and recurring motifs. Stateless.
pub async fn machines_harmony_handler(Json(req): Json<HarmonyRequest>) -> Json<serde_json::Value> {
    use crate::machines::harmony;
    let len = req.motif_len.unwrap_or(3);
    let intervals = harmony::intervals(&req.notes);
    let motifs: Vec<serde_json::Value> = harmony::motifs(&req.notes, len)
        .into_iter()
        .map(|(m, c)| serde_json::json!({ "intervals": m, "count": c }))
        .collect();
    Json(serde_json::json!({ "intervals": intervals, "motifs": motifs }))
}

/// POST /api/v1/machines/polis — reduce directed social interactions to
/// reciprocity-weighted ties; flag strengthening vs fading. Stateless.
pub async fn machines_polis_handler(Json(req): Json<PolisRequest>) -> Json<serde_json::Value> {
    use crate::machines::polis;
    let interactions: Vec<polis::Interaction> = req
        .interactions
        .iter()
        .map(|i| polis::Interaction {
            from: i.from.clone(),
            to: i.to.clone(),
            weight: i.weight,
        })
        .collect();
    let ties = polis::ties(&interactions);
    let min_s = req.min_strength.unwrap_or(0.0);
    let min_r = req.min_reciprocity.unwrap_or(0.5);
    let max_r = req.max_reciprocity.unwrap_or(0.5);
    let tie_json = |t: &polis::Tie| serde_json::json!({ "a": t.a, "b": t.b, "strength": t.strength, "reciprocity": t.reciprocity });
    let all: Vec<serde_json::Value> = ties.iter().map(tie_json).collect();
    let strengthening: Vec<serde_json::Value> = polis::strengthening(&ties, min_s, min_r)
        .into_iter()
        .map(tie_json)
        .collect();
    let fading: Vec<serde_json::Value> = polis::fading(&ties, max_r)
        .into_iter()
        .map(tie_json)
        .collect();
    Json(serde_json::json!({ "ties": all, "strengthening": strengthening, "fading": fading }))
}

/// POST /api/v1/machines/erosp — Platonic eros (Penia × Poros): reduce directed
/// reaches to ascending longings; flag ascending (drive + resource) vs hopeless
/// (drive, no resource). Stateless.
pub async fn machines_erosp_handler(Json(req): Json<ErospRequest>) -> Json<serde_json::Value> {
    use crate::machines::erosp;
    let reaches = to_eros_reaches(&req.reaches);
    let longings = erosp::detect_longings(&reaches);
    let min_i = req.min_intensity.unwrap_or(0.0);
    let min_a = req.min_ascent.unwrap_or(0.1);
    let max_a = req.max_ascent.unwrap_or(0.05);
    let lj = |l: &erosp::Longing| {
        serde_json::json!({
            "subject": l.subject, "object": l.object,
            "penia": l.penia, "poros": l.poros,
            "intensity": l.intensity, "ascent": l.ascent,
        })
    };
    let all: Vec<serde_json::Value> = longings.iter().map(lj).collect();
    let ascending: Vec<serde_json::Value> = erosp::ascending(&longings, min_i, min_a)
        .into_iter()
        .map(lj)
        .collect();
    let hopeless: Vec<serde_json::Value> = erosp::hopeless(&longings, max_a)
        .into_iter()
        .map(lj)
        .collect();
    Json(serde_json::json!({ "longings": all, "ascending": ascending, "hopeless": hopeless }))
}

/// POST /api/v1/machines/erosa — mythic eros (Ares × Aphrodite): reduce directed
/// reaches to conquests (valid only as child of love AND strife); flag passionate
/// (balanced) vs jealous (strife overruns love). Stateless.
pub async fn machines_erosa_handler(Json(req): Json<ErosaRequest>) -> Json<serde_json::Value> {
    use crate::machines::erosa;
    let reaches = to_eros_reaches(&req.reaches);
    let conquests = erosa::detect_conquests(&reaches);
    let min_b = req.min_balance.unwrap_or(0.5);
    let cj = |c: &erosa::Conquest| {
        serde_json::json!({
            "subject": c.subject, "object": c.object,
            "aphrodite": c.aphrodite, "ares": c.ares,
            "rivals": c.rivals, "balance": c.balance,
        })
    };
    let all: Vec<serde_json::Value> = conquests.iter().map(cj).collect();
    let passionate: Vec<serde_json::Value> = erosa::passionate(&conquests, min_b)
        .into_iter()
        .map(cj)
        .collect();
    let jealous: Vec<serde_json::Value> = erosa::jealous(&conquests).into_iter().map(cj).collect();
    Json(serde_json::json!({ "conquests": all, "passionate": passionate, "jealous": jealous }))
}

/// POST /api/v1/machines/techne — reduce a set of source files to capabilities
/// and module dependency edges; optionally compute the blast radius of a module.
/// Stateless.
pub async fn machines_techne_handler(Json(req): Json<TechneRequest>) -> Json<serde_json::Value> {
    use crate::machines::techne;
    let files: Vec<techne::SourceFile> = req
        .files
        .iter()
        .map(|f| techne::SourceFile {
            path: f.path.clone(),
            lines: f.lines.clone(),
        })
        .collect();
    let caps: Vec<serde_json::Value> = techne::classify_capabilities(&files)
        .iter()
        .map(|c| serde_json::json!({ "name": c.name, "files": c.files }))
        .collect();
    let edges = techne::dependency_edges(&files);
    let deps: Vec<serde_json::Value> = edges
        .iter()
        .map(|(a, b)| serde_json::json!([a, b]))
        .collect();
    let impact = req
        .impact_target
        .as_ref()
        .map(|t| techne::impact_of(&edges, t));
    Json(serde_json::json!({ "capabilities": caps, "dependencies": deps, "impact": impact }))
}

/// POST /api/v1/machines/euclid — reduce closed forms to geometric primitives
/// (Form vs Noise by circularity), and relate them (contained / near). Stateless.
pub async fn machines_euclid_handler(Json(req): Json<EuclidRequest>) -> Json<serde_json::Value> {
    use crate::machines::euclid;
    let limit = req.circularity_limit.unwrap_or(0.85);
    let near = req.near.unwrap_or(50.0);
    let prims: Vec<euclid::Primitive> = req
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
    let reduced: Vec<serde_json::Value> = prims
        .iter()
        .map(|p| {
            let st = euclid::classify(p, limit);
            serde_json::json!({
                "shape": euclid::classify_by_vertices(p.vertices),
                "status": if matches!(st, euclid::Status::Form) { "form" } else { "noise" },
                "coherence": st.coherence(),
                "hash": euclid::compute_hash(p),
            })
        })
        .collect();
    let mut relations = Vec::new();
    for i in 0..prims.len() {
        for j in (i + 1)..prims.len() {
            let rel = euclid::relation(&prims[i], &prims[j], near);
            let name = match rel {
                euclid::Relation::Contained => "contained",
                euclid::Relation::Near => "near",
                euclid::Relation::Disjoint => continue,
            };
            relations.push(serde_json::json!({ "a": i, "b": j, "relation": name }));
        }
    }
    Json(serde_json::json!({ "primitives": reduced, "relations": relations }))
}

/// POST /api/v1/machines/plutos — reduce transactions to recurring value flows;
/// compose per-party balances; surface the largest drains and the breaching
/// (net-negative) parties. Stateless.
pub async fn machines_plutos_handler(Json(req): Json<PlutosRequest>) -> Json<serde_json::Value> {
    use crate::machines::plutos;
    let txns: Vec<plutos::Transaction> = req
        .transactions
        .iter()
        .map(|t| plutos::Transaction {
            from: t.from.clone(),
            to: t.to.clone(),
            amount: t.amount,
        })
        .collect();
    let flows = plutos::detect_flows(&txns, req.min_count.unwrap_or(2));
    let bals = plutos::balances(&flows);
    let fj = |f: &plutos::Flow| {
        serde_json::json!({
            "from": f.from, "to": f.to, "total": f.total, "count": f.count, "mean": f.mean,
        })
    };
    let bj = |b: &plutos::Balance| {
        serde_json::json!({
            "party": b.party, "inflow": b.inflow, "outflow": b.outflow, "net": b.net,
        })
    };
    let flows_json: Vec<serde_json::Value> = flows.iter().map(fj).collect();
    let drains: Vec<serde_json::Value> = plutos::drains(&flows, req.min_total.unwrap_or(0.0))
        .into_iter()
        .map(fj)
        .collect();
    let balances_json: Vec<serde_json::Value> = bals.iter().map(bj).collect();
    let breaches: Vec<serde_json::Value> = plutos::breaches(&bals).into_iter().map(bj).collect();
    Json(serde_json::json!({
        "flows": flows_json,
        "drains": drains,
        "balances": balances_json,
        "breaches": breaches,
    }))
}

/// POST /api/v1/machines/krisis — reduce pairwise choices to net preferences,
/// rank them (Copeland), and flag intransitive cycles (the incoherent taste).
/// Stateless.
pub async fn machines_krisis_handler(Json(req): Json<KrisisRequest>) -> Json<serde_json::Value> {
    use crate::machines::krisis;
    let choices: Vec<krisis::Choice> = req
        .choices
        .iter()
        .map(|c| krisis::Choice {
            over: c.over.clone(),
            under: c.under.clone(),
        })
        .collect();
    let prefs = krisis::detect_preferences(&choices);
    let prefs_json: Vec<serde_json::Value> = prefs
        .iter()
        .map(|p| serde_json::json!({ "over": p.over, "under": p.under, "margin": p.margin }))
        .collect();
    let ranking: Vec<serde_json::Value> = krisis::ranking(&prefs)
        .iter()
        .map(|r| serde_json::json!({ "option": r.option, "score": r.score }))
        .collect();
    let cycles: Vec<serde_json::Value> = krisis::cycles(&prefs)
        .into_iter()
        .map(|c| serde_json::json!(c))
        .collect();
    Json(serde_json::json!({ "preferences": prefs_json, "ranking": ranking, "cycles": cycles }))
}

/// POST /api/v1/machines/pathos — reduce affect episodes to recurring patterns
/// (flat episodes rejected); mark amplifiers and soothers. Stateless.
pub async fn machines_pathos_handler(Json(req): Json<PathosRequest>) -> Json<serde_json::Value> {
    use crate::machines::pathos;
    let episodes: Vec<pathos::Episode> = req
        .episodes
        .iter()
        .map(|e| pathos::Episode {
            trigger: e.trigger.clone(),
            valence: e.valence,
            arousal: e.arousal,
        })
        .collect();
    let affects = pathos::detect_affects(
        &episodes,
        req.min_count.unwrap_or(2),
        req.flat.unwrap_or(0.1),
    );
    let aj = |a: &pathos::Affect| serde_json::json!({ "trigger": a.trigger, "valence": a.valence, "arousal": a.arousal, "count": a.count });
    let all: Vec<serde_json::Value> = affects.iter().map(aj).collect();
    let amplifiers: Vec<serde_json::Value> = pathos::amplifiers(&affects, 0.5)
        .into_iter()
        .map(aj)
        .collect();
    let soothers: Vec<serde_json::Value> = pathos::soothers(&affects, 0.5)
        .into_iter()
        .map(aj)
        .collect();
    Json(serde_json::json!({ "affects": all, "amplifiers": amplifiers, "soothers": soothers }))
}

/// POST /api/v1/machines/chroma — reduce colors to saturated hues (muddy grey
/// rejected) and relate them on the wheel (complement/analogous/triad/clash).
/// Stateless.
pub async fn machines_chroma_handler(Json(req): Json<ChromaRequest>) -> Json<serde_json::Value> {
    use crate::machines::chroma;
    let colors: Vec<chroma::Color> = req
        .colors
        .iter()
        .map(|c| chroma::Color {
            r: c.r,
            g: c.g,
            b: c.b,
        })
        .collect();
    let min_chroma = req.min_chroma.unwrap_or(0.2);
    let tol = req.tol.unwrap_or(20.0);
    let rels = chroma::detect_relations(&colors, min_chroma, tol);
    let rj = |r: &chroma::Relation| {
        let kind = match r.kind {
            chroma::Kind::Complement => "complement",
            chroma::Kind::Analogous => "analogous",
            chroma::Kind::Triad => "triad",
            chroma::Kind::Clash => "clash",
        };
        serde_json::json!({ "a": r.a, "b": r.b, "kind": kind })
    };
    let all: Vec<serde_json::Value> = rels.iter().map(rj).collect();
    let harmonies: Vec<serde_json::Value> = chroma::harmonies(&rels).into_iter().map(rj).collect();
    let clashes: Vec<serde_json::Value> = chroma::clashes(&rels).into_iter().map(rj).collect();
    Json(serde_json::json!({ "relations": all, "harmonies": harmonies, "clashes": clashes }))
}

/// POST /api/v1/machines/kinesis — reduce gesture trajectories to motion gestalts
/// (sub-threshold jitter rejected); mark reaches and degrading gaits. Stateless.
pub async fn machines_kinesis_handler(Json(req): Json<KinesisRequest>) -> Json<serde_json::Value> {
    use crate::machines::kinesis;
    let gestures: Vec<kinesis::Gesture> = req
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
    let motions = kinesis::detect_motions(&gestures, req.min_move.unwrap_or(0.5));
    let mj = |m: &kinesis::Motion| {
        serde_json::json!({
            "label": m.label, "path": m.path, "net": m.net,
            "straightness": m.straightness, "moves": m.moves,
        })
    };
    let all: Vec<serde_json::Value> = motions.iter().map(mj).collect();
    let reaches: Vec<serde_json::Value> = kinesis::reaches(&motions, req.min_net.unwrap_or(1.0))
        .into_iter()
        .map(mj)
        .collect();
    let degrading = kinesis::degrading(&motions, req.drop.unwrap_or(0.2));
    Json(serde_json::json!({ "motions": all, "reaches": reaches, "degrading": degrading }))
}

/// POST /api/v1/machines/soma — classify training sessions as effort/recovery/
/// overreaching and compute load drift over a window. Stateless.
pub async fn machines_soma_handler(Json(req): Json<SomaRequest>) -> Json<serde_json::Value> {
    use crate::machines::soma;
    let sessions: Vec<soma::Session> = req
        .sessions
        .iter()
        .map(|s| soma::Session {
            day: s.day,
            activity: s.activity.clone(),
            load: s.load,
            recovery: s.recovery,
        })
        .collect();
    let classified: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            let label = match soma::classify(s) {
                soma::Status::Effort => "effort",
                soma::Status::Recovery => "recovery",
                soma::Status::Overreaching => "overreaching",
            };
            serde_json::json!({ "day": s.day, "activity": s.activity, "status": label })
        })
        .collect();
    let window = req.window.unwrap_or(7);
    let drift = soma::drift(&sessions, window);
    Json(serde_json::json!({ "sessions": classified, "drift": drift }))
}

/// POST /api/v1/machines/elenchus — reduce a dialogue to Toulmin claim/warrant/
/// rebuttal primitives; return each claim's standing + the unsupported/contested
/// claims to press. Statements with an unknown kind are ignored. Stateless.
pub async fn machines_elenchus_handler(
    Json(req): Json<ElenchusRequest>,
) -> Json<serde_json::Value> {
    use crate::machines::elenchus;
    let statements: Vec<elenchus::Statement> = req
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
    let standings: Vec<serde_json::Value> = elenchus::evaluate(&statements)
        .into_iter()
        .map(|(id, st)| {
            let label = match st {
                elenchus::Standing::Supported => "supported",
                elenchus::Standing::Contested => "contested",
                elenchus::Standing::Unsupported => "unsupported",
            };
            serde_json::json!({ "claim": id, "standing": label })
        })
        .collect();
    Json(serde_json::json!({
        "standings": standings,
        "unsupported": elenchus::unsupported_claims(&statements),
        "contested": elenchus::contested_claims(&statements),
    }))
}

/// POST /api/v1/machines/lex — reduce contract clauses to obligation/right/
/// condition primitives; return unconditional obligations, breach paths for the
/// given failed conditions, and dangling (undefined) conditions. Stateless.
pub async fn machines_lex_handler(Json(req): Json<LexRequest>) -> Json<serde_json::Value> {
    use crate::machines::lex;
    let clauses: Vec<lex::Clause> = req
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
    Json(serde_json::json!({
        "unconditional_obligations": lex::unconditional_obligations(&clauses),
        "breach_paths": lex::breach_paths(&clauses, &req.failed),
        "dangling_conditions": lex::dangling_conditions(&clauses),
    }))
}

/// POST /api/v1/machines/apodeixis — check Lean proof obligations against the
/// kernel. Each obligation carries a natural-language `claim` plus a `.lean`
/// `snippet` (theorem + proof) seeded by a proof-engine model; apodeixis runs
/// it through the Lean kernel and returns `certified` / `refuted` / `untestable`
/// verdicts with the kernel's witness. Refuted obligations feed back into the
/// quality loop as embedded failures — claims beaten by the kernel are recorded,
/// not just reported.
pub async fn machines_apodeixis_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ApodeixisRequest>,
) -> Json<serde_json::Value> {
    use crate::machines::apodeixis;
    let obligations: Vec<apodeixis::Obligation> = req
        .obligations
        .iter()
        .map(|o| apodeixis::Obligation {
            id: o.id.clone(),
            claim: o.claim.clone(),
            snippet: o.snippet.clone(),
        })
        .collect();
    let checks = apodeixis::reduce(&obligations, req.timeout_secs.unwrap_or(30));
    let cj: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id, "claim": c.claim, "verdict": c.verdict.as_str(),
                "witness": c.witness, "exit_code": c.exit_code, "engine": c.engine,
            })
        })
        .collect();
    let rep = apodeixis::evaluate(&checks);

    // Hard verdicts → coherence nodes in the caller's tenant, and refutations
    // → quality tracker as embedded failures the loop learns from.
    let uid = user_key(&headers);
    let mut coherence_ids = Vec::new();
    let centroids = state.shared.classifier.cell_centroids();
    for c in checks
        .iter()
        .filter(|c| c.verdict != apodeixis::Verdict::Untestable)
    {
        let label = format!("apodeixis {}: {}", c.verdict.as_str(), c.claim);
        let embedding = embed_text(state.shared.clone(), label.clone()).await;
        let id = state.with_user(&uid, |s| {
            let id = s.core.register_node_vec_labeled(embedding, Some(label));
            s.core.assert_coherence(&id, c.verdict.coherence());
            id
        });
        coherence_ids.push(id);
        if c.verdict == apodeixis::Verdict::Refuted {
            let _ = state
                .quality
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .report_failure(&c.claim, &centroids, None);
        }
    }

    Json(serde_json::json!({
        "checks": cj,
        "certified": rep.certified,
        "refuted": rep.refuted,
        "untestable": rep.untestable,
        "coherence_ids": coherence_ids,
    }))
}

/// POST /api/v1/machines/empedocles — classify telemetry readings into the four
/// elements (earth/air/fire/water) and flag stable vs anomaly past threshold;
/// optional trend slope over a series. Stateless.
pub async fn machines_empedocles_handler(
    Json(req): Json<EmpedoclesRequest>,
) -> Json<serde_json::Value> {
    use crate::machines::empedocles;
    let threshold = req.threshold.unwrap_or(1.0);
    let readings: Vec<serde_json::Value> = req
        .readings
        .iter()
        .map(|r| {
            let reading = empedocles::Reading { metric: r.metric.clone(), value: r.value, baseline: r.baseline };
            let element = match reading.element() {
                empedocles::Element::Earth => "earth",
                empedocles::Element::Air => "air",
                empedocles::Element::Fire => "fire",
                empedocles::Element::Water => "water",
            };
            let status = match empedocles::classify(&reading, threshold) {
                empedocles::Status::Stable => "stable",
                empedocles::Status::Anomaly => "anomaly",
            };
            serde_json::json!({ "metric": r.metric, "element": element, "deviation": reading.deviation(), "status": status })
        })
        .collect();
    let trend = req.series.as_ref().map(|s| empedocles::trend(s));
    Json(serde_json::json!({ "readings": readings, "trend": trend }))
}

/// POST /api/v1/machines/topos — reduce geotagged waypoints to recurring places
/// (clustered, visited >= min_visits) and flag fading ones (not seen within
/// stale_days). Stateless.
pub async fn machines_topos_handler(Json(req): Json<ToposRequest>) -> Json<serde_json::Value> {
    use crate::machines::topos;
    let waypoints: Vec<topos::Waypoint> = req
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

    let radius = req.radius_km.unwrap_or(0.5);
    let min_visits = req.min_visits.unwrap_or(2);
    let stale = req.stale_days.unwrap_or(30);

    let places = topos::detect_places(&waypoints, radius, min_visits);
    let now = chrono::Utc::now();
    let place_json = |p: &topos::Place| {
        serde_json::json!({
            "lat": p.lat,
            "lng": p.lng,
            "label": p.label,
            "visits": p.visits,
            "last": p.last.map(|d| d.to_rfc3339()),
            "radius_km": p.radius_km,
        })
    };
    let all: Vec<serde_json::Value> = places.iter().map(place_json).collect();
    let fading: Vec<serde_json::Value> = topos::fading(&places, now, stale)
        .into_iter()
        .map(place_json)
        .collect();

    Json(serde_json::json!({ "places": all, "fading": fading }))
}
