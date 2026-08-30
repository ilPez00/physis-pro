//! Monitor mode — a phone's sensors, translated into what the engine already
//! reduces (PLAN Phase 14).
//!
//! A day leaves traces the person living it never writes down. The step counter
//! knows there was a workout; the GPS knows it was the gym and not the office;
//! the clock knows it was the fourth Tuesday in a row and that the fifth did not
//! happen. None of that reaches the graph today.
//!
//! # This is a mapping layer, not an engine
//!
//! The split it needs is the one Phase 8b closed: the device **perceives**, the
//! engine **means**. So there is nothing here that reduces, scores or decides —
//! every one of those already exists and is tested:
//!
//! * `kinesis` reduces motion, `soma` classifies effort / recovery / overreach,
//! * `topos` reduces fixes to *recurring places* rather than a trace,
//! * `chronos` finds rhythms and, more usefully, the ones that are slipping,
//! * `empedocles` reads a value against its baseline.
//!
//! What this module owns is the translation, and it owns it **server-side on
//! purpose**: which sensor becomes which primitive is a product decision, and
//! re-implementing it in every client is how two clients end up disagreeing
//! about what a morning was.
//!
//! # What it deliberately refuses to carry
//!
//! [`SensorSample`] has no field for audio, no field for a transcript, and no
//! field for raw coordinates history beyond what `topos` needs to find a place.
//! Recording yourself is the user's business; a lesson or a call captures other
//! people, and the liability lives in *storing the audio* while the value lives
//! in the inference — which does not need it. An audio sample arrives here as
//! `voices: Option<u8>` and a duration, never as sound. See
//! `docs/plans/2026-08-30-monitor-mode.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One reading from a device sensor.
///
/// Untagged-ish by design: a phone batches heterogeneous samples and posting
/// four differently-shaped arrays per sync is four chances for them to drift
/// out of order. Every sample carries its own kind and its own timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SensorSample {
    /// Steps counted since the previous sample of this kind.
    Steps {
        at: DateTime<Utc>,
        count: u32,
        /// Minutes the count covers; used to separate a walk from a workout.
        #[serde(default = "default_window")]
        minutes: u32,
    },
    /// A location fix. Accuracy is kept so a bad fix can be dropped before it
    /// invents a place nobody visited.
    Location {
        at: DateTime<Utc>,
        lat: f64,
        lng: f64,
        #[serde(default)]
        accuracy_m: Option<f32>,
        /// A name the device already knows (a saved place, a geofence).
        #[serde(default)]
        label: Option<String>,
    },
    /// A period during which speech was detected — **never the speech**.
    /// `voices` is an on-device estimate; 2 or more means other people were
    /// present, which is a fact about the calendar, not a recording of them.
    Audio {
        at: DateTime<Utc>,
        minutes: u32,
        #[serde(default)]
        voices: Option<u8>,
    },
    /// Any scalar the device tracks against a baseline: battery, heart rate,
    /// ambient light, screen-on minutes.
    Reading {
        at: DateTime<Utc>,
        metric: String,
        value: f32,
        baseline: f32,
    },
}

fn default_window() -> u32 {
    60
}

impl SensorSample {
    pub fn at(&self) -> DateTime<Utc> {
        match self {
            SensorSample::Steps { at, .. }
            | SensorSample::Location { at, .. }
            | SensorSample::Audio { at, .. }
            | SensorSample::Reading { at, .. } => *at,
        }
    }
}

/// A batch from one device, one sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorBatch {
    /// Which device this came from. Two phones are two witnesses, and a place
    /// seen by both is not the same evidence as a place seen by one.
    pub device: String,
    pub samples: Vec<SensorSample>,
}

/// Steps per hour above which a stretch counts as a workout rather than a walk.
///
/// A brisk walk runs 4–6k steps/hour; sustained running is well past 8k. The
/// threshold is deliberately generous: calling a long walk a workout is a small
/// error, and calling a workout "nothing" loses the event entirely.
pub const WORKOUT_STEPS_PER_HOUR: f32 = 4_500.0;

/// A fix worse than this is dropped rather than clustered. Consumer GPS in a
/// building reports hundreds of metres of error, and a place invented out of
/// that error is a place the user never went — worse than a gap.
pub const MAX_FIX_ACCURACY_M: f32 = 120.0;

/// What one batch turns into, before any of it reaches a reducer.
///
/// Kept as a value (rather than pushed straight into the machines) so the
/// translation is testable on its own and so the HTTP layer can report what it
/// understood — a monitor that silently discards half a morning is worse than
/// one that says it kept three of seven samples.
/// Not `Serialize`: the machine primitives it holds are internal types, and a
/// response that echoed every waypoint back to the phone would be sending the
/// device its own location history for no reason. [`Translation::summary`] is
/// what the HTTP layer reports.
#[derive(Debug, Clone, Default)]
pub struct Translation {
    /// For `soma`: one session per activity stretch.
    pub sessions: Vec<crate::machines::soma::Session>,
    /// For `topos`: fixes good enough to cluster into places.
    pub waypoints: Vec<crate::machines::topos::Waypoint>,
    /// For `chronos`: everything worth asking "does this recur?" about.
    pub events: Vec<crate::machines::chronos::TimedEvent>,
    /// For `empedocles`: scalars against their baseline.
    pub readings: Vec<crate::machines::empedocles::Reading>,
    /// Samples that were understood but deliberately not forwarded, and why.
    /// Reported, never silent.
    pub skipped: Vec<String>,
}

/// What the caller is told: how much of the batch was understood, and what was
/// not. Counts, never contents.
#[derive(Debug, Clone, Serialize)]
pub struct TranslationSummary {
    pub sessions: usize,
    pub waypoints: usize,
    pub events: usize,
    pub readings: usize,
    pub skipped: Vec<String>,
}

impl Translation {
    pub fn summary(&self) -> TranslationSummary {
        TranslationSummary {
            sessions: self.sessions.len(),
            waypoints: self.waypoints.len(),
            events: self.events.len(),
            readings: self.readings.len(),
            skipped: self.skipped.clone(),
        }
    }

    /// True when nothing in the batch reached a reducer. Worth reporting: a
    /// phone syncing every ten minutes and translating to nothing every time is
    /// a misconfigured collector, not a quiet day.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
            && self.waypoints.is_empty()
            && self.events.is_empty()
            && self.readings.is_empty()
    }
}

/// The label an observation is remembered under.
///
/// `chronos` keeps **rhythms**, not events: three lessons on three Tuesdays
/// become "lesson every 7d", and a single lesson becomes nothing at all. That
/// is right for chronos and wrong for a monitor — "there was a 50-minute
/// conversation at 14:00" is exactly the fact the day is worth recording for,
/// and feeding only chronos silently loses every first occurrence.
///
/// So each observation is also remembered on its own, and the timestamp is part
/// of the label because registration dedupes by label: without it, Tuesday's
/// lesson and Wednesday's would collapse into one node and the second day would
/// look like it never happened.
pub fn observation_label(event: &crate::machines::chronos::TimedEvent) -> String {
    format!("{} · {}", event.label, event.at.format("%Y-%m-%d %H:%M"))
}

/// Translate one batch into the primitives the machines consume.
///
/// Pure: no clock, no store, no network. The same batch always produces the
/// same primitives, which is what makes a re-sync after a dropped connection
/// safe to replay.
pub fn translate(batch: &SensorBatch) -> Translation {
    let mut out = Translation::default();

    for sample in &batch.samples {
        match sample {
            SensorSample::Steps { at, count, minutes } => {
                let minutes = (*minutes).max(1);
                let per_hour = *count as f32 * 60.0 / minutes as f32;
                // Every stretch of movement is a rhythm candidate; only the
                // intense ones are sessions. A daily walk is exactly the kind
                // of thing `chronos` should notice stopping.
                let activity = if per_hour >= WORKOUT_STEPS_PER_HOUR {
                    "workout"
                } else if *count > 0 {
                    "walk"
                } else {
                    out.skipped
                        .push(format!("{}: no steps, nothing to record", at.to_rfc3339()));
                    continue;
                };
                out.events.push(crate::machines::chronos::TimedEvent {
                    label: activity.to_string(),
                    at: *at,
                });
                if activity == "workout" {
                    // `soma`'s load/recovery are normalized 0..1. Load comes
                    // from intensity relative to the workout threshold, capped:
                    // a marathon and a very hard hour are both "1.0 hard", and
                    // pretending to resolve past that would be invention.
                    let load = (per_hour / (WORKOUT_STEPS_PER_HOUR * 2.0)).clamp(0.0, 1.0);
                    out.sessions.push(crate::machines::soma::Session {
                        // Day-of-year: `soma` orders sessions and compares
                        // halves of a window; it needs an ordinal, not a date.
                        day: at.format("%j").to_string().parse().unwrap_or(0),
                        activity: activity.to_string(),
                        load,
                        // Recovery is not observable from a step counter. It is
                        // left neutral rather than guessed, which keeps a
                        // hard session out of "overreaching" until something
                        // that can actually see recovery reports it.
                        recovery: 0.5,
                    });
                }
            }

            SensorSample::Location {
                at,
                lat,
                lng,
                accuracy_m,
                label,
            } => {
                if let Some(acc) = accuracy_m {
                    if *acc > MAX_FIX_ACCURACY_M {
                        out.skipped.push(format!(
                            "{}: fix accurate to only {acc:.0}m, dropped (a place invented from \
                             error is a place you never went)",
                            at.to_rfc3339()
                        ));
                        continue;
                    }
                }
                out.waypoints.push(crate::machines::topos::Waypoint {
                    lat: *lat,
                    lng: *lng,
                    label: label.clone(),
                    at: Some(*at),
                });
            }

            SensorSample::Audio {
                at,
                minutes,
                voices,
            } => {
                // The event names what happened, not what was said. With more
                // than one voice it was a conversation; alone it was dictation
                // or a call on speaker — either way the audio itself never
                // reaches this function, by the shape of the type.
                let label = match voices {
                    Some(v) if *v >= 2 => format!("conversation ({v} voices)"),
                    Some(1) => "speech (one voice)".to_string(),
                    _ => "speech".to_string(),
                };
                out.events.push(crate::machines::chronos::TimedEvent {
                    label: label.clone(),
                    at: *at,
                });
                // Duration is a scalar worth trending: a week of 8-minute calls
                // and a week of 90-minute lectures are different weeks.
                out.readings.push(crate::machines::empedocles::Reading {
                    metric: "speech_minutes".to_string(),
                    value: *minutes as f32,
                    baseline: 30.0,
                });
            }

            SensorSample::Reading {
                metric,
                value,
                baseline,
                ..
            } => {
                out.readings.push(crate::machines::empedocles::Reading {
                    metric: metric.clone(),
                    value: *value,
                    baseline: *baseline,
                });
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&format!("2026-08-30T{hour:02}:00:00Z"))
            .unwrap()
            .with_timezone(&Utc)
    }

    fn batch(samples: Vec<SensorSample>) -> SensorBatch {
        SensorBatch {
            device: "pixel".into(),
            samples,
        }
    }

    #[test]
    fn a_burst_of_steps_is_a_workout_and_a_stroll_is_not() {
        let t = translate(&batch(vec![
            SensorSample::Steps {
                at: at(7),
                count: 6000,
                minutes: 45,
            },
            SensorSample::Steps {
                at: at(13),
                count: 900,
                minutes: 60,
            },
        ]));
        // Both are rhythm candidates — a daily walk stopping is exactly what
        // chronos should notice — but only the burst is a session.
        assert_eq!(t.events.len(), 2, "{:?}", t.events);
        assert_eq!(t.sessions.len(), 1, "{:?}", t.sessions);
        assert_eq!(t.sessions[0].activity, "workout");
        assert!(t.events.iter().any(|e| e.label == "walk"));
    }

    #[test]
    fn recovery_is_left_neutral_because_a_step_counter_cannot_see_it() {
        // Guessing recovery from step count would push every hard session into
        // soma's "overreaching" band on no evidence at all.
        let t = translate(&batch(vec![SensorSample::Steps {
            at: at(18),
            count: 9000,
            minutes: 40,
        }]));
        let s = &t.sessions[0];
        assert_eq!(s.recovery, 0.5);
        assert_eq!(
            crate::machines::soma::classify(s),
            crate::machines::soma::Status::Effort,
            "a hard session with unknown recovery is effort, not overreach"
        );
    }

    #[test]
    fn load_saturates_rather_than_inventing_resolution() {
        let hard = translate(&batch(vec![SensorSample::Steps {
            at: at(6),
            count: 20_000,
            minutes: 60,
        }]));
        let harder = translate(&batch(vec![SensorSample::Steps {
            at: at(6),
            count: 60_000,
            minutes: 60,
        }]));
        assert_eq!(hard.sessions[0].load, 1.0);
        assert_eq!(harder.sessions[0].load, 1.0, "both are simply 'hard'");
    }

    #[test]
    fn a_bad_fix_is_dropped_and_said_so() {
        let t = translate(&batch(vec![
            SensorSample::Location {
                at: at(9),
                lat: 45.46,
                lng: 9.19,
                accuracy_m: Some(15.0),
                label: Some("plant".into()),
            },
            SensorSample::Location {
                at: at(10),
                lat: 45.99,
                lng: 9.99,
                accuracy_m: Some(800.0),
                label: None,
            },
        ]));
        assert_eq!(t.waypoints.len(), 1, "{:?}", t.waypoints);
        assert_eq!(t.waypoints[0].label.as_deref(), Some("plant"));
        assert_eq!(t.skipped.len(), 1, "a dropped sample is reported");
        assert!(t.skipped[0].contains("800m"), "{:?}", t.skipped);
    }

    #[test]
    fn audio_becomes_an_event_and_a_duration_never_a_recording() {
        let t = translate(&batch(vec![SensorSample::Audio {
            at: at(14),
            minutes: 50,
            voices: Some(3),
        }]));
        assert_eq!(t.events.len(), 1);
        assert_eq!(t.events[0].label, "conversation (3 voices)");
        let r = &t.readings[0];
        assert_eq!(r.metric, "speech_minutes");
        assert_eq!(r.value, 50.0);

        // The guarantee is structural, not a rule someone remembers: there is
        // nowhere in the serialized sample for audio to live.
        let json = serde_json::to_string(&SensorSample::Audio {
            at: at(14),
            minutes: 50,
            voices: Some(3),
        })
        .unwrap();
        assert!(!json.contains("audio_"), "{json}");
        assert!(!json.contains("transcript"), "{json}");
        assert!(!json.contains("pcm"), "{json}");
    }

    #[test]
    fn an_observation_label_carries_its_time_so_two_days_are_two_facts() {
        // Registration dedupes by label. Without the timestamp, Tuesday's
        // lesson and Wednesday's collapse into one node and the second day
        // looks like it never happened.
        let tue = crate::machines::chronos::TimedEvent {
            label: "conversation (3 voices)".into(),
            at: at(14),
        };
        let wed = crate::machines::chronos::TimedEvent {
            label: "conversation (3 voices)".into(),
            at: at(15),
        };
        assert_ne!(observation_label(&tue), observation_label(&wed));
        assert!(observation_label(&tue).starts_with("conversation (3 voices)"));
        // The bare label stays bare, because that is what chronos matches on to
        // find the rhythm across days.
        assert_eq!(tue.label, wed.label);
    }

    #[test]
    fn translation_is_pure_so_a_resync_can_be_replayed() {
        // A phone that loses connectivity mid-sync retries the same batch.
        // Same input, same primitives — the dedup downstream then does its job.
        let b = batch(vec![
            SensorSample::Steps {
                at: at(7),
                count: 6000,
                minutes: 45,
            },
            SensorSample::Reading {
                at: at(7),
                metric: "battery".into(),
                value: 42.0,
                baseline: 80.0,
            },
        ]);
        let one = translate(&b);
        let two = translate(&b);
        assert_eq!(format!("{:?}", one.sessions), format!("{:?}", two.sessions));
        assert_eq!(format!("{:?}", one.readings), format!("{:?}", two.readings));
    }

    #[test]
    fn an_empty_day_produces_nothing_rather_than_a_zero() {
        // A phone left on a desk should not register "0 steps" as an event;
        // the graph would fill with non-events and chronos would find a rhythm
        // in a drawer.
        let t = translate(&batch(vec![SensorSample::Steps {
            at: at(3),
            count: 0,
            minutes: 60,
        }]));
        assert!(t.events.is_empty());
        assert!(t.sessions.is_empty());
        assert_eq!(t.skipped.len(), 1);
    }
}
