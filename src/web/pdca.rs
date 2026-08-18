use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, State}, http::{StatusCode, HeaderMap}};

pub async fn ingest_prompt_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Json<IngestResponse> {
    let prompt = req
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let embedding = embed_text(state.shared.clone(), prompt.clone()).await;
    // Retain the prompt: the vector is the state, but without the text every
    // dream proposal over this goal can only say "unknown".
    let mut goal = Goal::new_vec(embedding).with_label(prompt.clone());
    goal.progress = 0.0;
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        s.goals.push(goal.clone());
        // Goals were the last tenant state still lost on restart.
        if let Some(db) = state.db() {
            if let Err(e) = save_goals(db, &uid, &s.goals) {
                log::warn!("goals not persisted for {uid}: {e}");
            }
        }
    });
    Json(IngestResponse {
        message: format!("Goal created from prompt ({} chars)", prompt.len()),
        goal,
    })
}

pub async fn pdca_plan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let planned = s.actor.plan(&s.goals);
        let goals: Vec<serde_json::Value> = planned
            .iter()
            .map(|g| {
                serde_json::json!({
                    "id": g.id,
                    "embedding": g.embedding,
                    "progress": g.progress,
                })
            })
            .collect();
        Json(serde_json::json!({ "planned": goals, "count": goals.len() }))
    })
}

pub async fn pdca_act_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<PDCARequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let before = match s.goals.iter().find(|g| g.id == req.goal_id) {
            Some(g) => g.embedding.clone(),
            None => return Json(serde_json::json!({"error": "goal not found"})),
        };
        let after = req.state_vector.clone();
        let exp_id = s.actor.do_action(&req.goal_id, before, after).id;
        let exps = s.actor.experiences.clone();
        let mut goals = std::mem::take(&mut s.goals);
        s.actor.act(&exps, &mut goals);
        s.goals = goals;
        Json(serde_json::json!({ "experience_id": exp_id }))
    })
}

pub async fn pdca_stats_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<crate::actor::PDCAStats> {
    let uid = user_key(&headers);
    Json(state.with_user(&uid, |s| s.actor.stats(&s.goals)))
}

pub async fn schedule_status_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let health = s.scheduler.schedule_health();
        let tasks: Vec<serde_json::Value> = s
            .scheduler
            .tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "duration": t.planned_duration_secs,
                    "early_start": t.early_start,
                    "early_finish": t.early_finish,
                    "late_start": t.late_start,
                    "late_finish": t.late_finish,
                    "float": t.float,
                    "critical": t.on_critical_path,
                    "progress": t.progress,
                })
            })
            .collect();
        let critical_path: Vec<&str> = s
            .scheduler
            .tasks
            .iter()
            .filter(|t| t.on_critical_path)
            .map(|t| t.id.as_str())
            .collect();
        let at_risk: Vec<&str> = s
            .scheduler
            .tasks_approaching_late_finish(3600)
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        Json(serde_json::json!({
            "schedule_health": health,
            "task_count": tasks.len(),
            "tasks": tasks,
            "critical_path": critical_path,
            "at_risk_tasks": at_risk,
        }))
    })
}

pub async fn schedule_cross_correlate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let progress_pairs: Vec<(String, f32)> =
            s.goals.iter().map(|g| (g.id.clone(), g.progress)).collect();
        let r = s.scheduler.cross_correlate(&progress_pairs);
        Json(serde_json::json!({
            "pearson_r": r,
            "interpretation": if r.abs() > 0.7 {
                "strong correlation"
            } else if r.abs() > 0.4 {
                "moderate correlation"
            } else {
                "weak or no correlation"
            }
        }))
    })
}

pub async fn schedule_add_task_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AddTaskRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        req.task_id.hash(&mut hasher);
        let node_key = crate::graph::RawNodeKey {
            data: hasher.finish(),
        };
        let deps: Vec<crate::graph::RawNodeKey> = req
            .dependencies
            .iter()
            .map(|d| {
                let mut h = DefaultHasher::new();
                d.hash(&mut h);
                crate::graph::RawNodeKey { data: h.finish() }
            })
            .collect();
        s.scheduler.add_task_with_duration(
            node_key,
            req.task_id.clone(),
            req.duration_secs.max(0.0) as u64,
            0,
            deps,
        );
        Json(serde_json::json!({ "status": "ok", "task_id": req.task_id }))
    })
}

pub async fn schedule_recompute_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        s.scheduler.recompute();
        Json(serde_json::json!({ "schedule_health": s.scheduler.schedule_health() }))
    })
}

/// Stateless CPM: caller supplies the full task list, gets the computed schedule
/// back. Holds no server state, so it behaves identically across replicas and on
/// a redeploy (unlike the in-memory `/schedule/*` routes above, which are
/// single-instance/local only).
pub async fn schedule_compute_handler(Json(req): Json<ScheduleComputeRequest>) -> Json<ScheduleResult> {
    Json(compute_schedule(
        &req.tasks,
        &req.goal_progress,
        req.domain.as_deref().unwrap_or("abstract"),
    ))
}

/// Insert or replace a LifeOS goal in the caller's tenant.
pub async fn lifeos_goal_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(goal): Json<LifeOSGoal>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let id = goal.id.clone();
    let db = state.db.clone();
    let replaced = state.with_user(&uid, |s| {
        let r = s.lifeos.upsert_goal(goal);
        if let Some(db) = &db {
            let _ = s.lifeos.save(db, &uid);
        }
        r
    });
    Json(serde_json::json!({ "status": "ok", "id": id, "replaced": replaced }))
}

/// Update a goal's progress.
pub async fn lifeos_progress_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<LifeOsProgressRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let uid = user_key(&headers);
    let db = state.db.clone();
    let ok = state.with_user(&uid, |s| {
        let r = s.lifeos.update_progress(&req.id, req.progress);
        if r {
            if let Some(db) = &db {
                let _ = s.lifeos.save(db, &uid);
            }
        }
        r
    });
    let code = if ok {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (code, Json(serde_json::json!({ "ok": ok, "id": req.id })))
}

/// Current goals + schedule-drift snapshot (replaces the old outbound push).
pub async fn lifeos_status_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<LifeOsStatus> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        // Reflect current goals in the schedule before reporting drift.
        let RuntimeState {
            lifeos, scheduler, ..
        } = s;
        lifeos.apply_to_scheduler(scheduler);
        scheduler.recompute();
        Json(LifeOsStatus {
            goals: lifeos.goals.clone(),
            drift: lifeos.drift_report(scheduler),
        })
    })
}

/// Set the current bioenergetic charge (Praxis derives it from check-ins).
pub async fn lifeos_charge_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ChargeRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let db = state.db.clone();
    state.with_user(&uid, |s| {
        s.lifeos.set_charge(req.charge);
        if let Some(db) = &db {
            let _ = s.lifeos.save(db, &uid);
        }
    });
    Json(serde_json::json!({ "ok": true }))
}

/// Charge-governed "today": goals matched to current charge + a bioenergetic
/// countermeasure when depleted/drifting. The keystone of the bioenergetics↔
/// lifeos merge.
pub async fn lifeos_today_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<TodayPlan> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let RuntimeState {
            lifeos, scheduler, ..
        } = s;
        lifeos.apply_to_scheduler(scheduler);
        scheduler.recompute();
        Json(lifeos.today(scheduler))
    })
}

