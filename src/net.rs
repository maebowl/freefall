use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::replay::FrameInput;

// Networking is native-only; these are unused when compiled for the web.
#[cfg(not(target_family = "wasm"))]
use crate::ldtk::CurrentLevel;
#[cfg(not(target_family = "wasm"))]
use crate::level::{GameMode, GamePhase};
#[cfg(not(target_family = "wasm"))]
use crate::replay::ReplayData;

const API_URL: &str = "https://freefall.mabelwallin.com/api";

// --- Public types ---

#[derive(Resource)]
pub struct PlayerName(pub String);

#[derive(Resource, Default)]
pub struct OnlineLeaderboard {
    pub entries: Vec<OnlineEntry>,
    pub status: NetStatus,
}

#[derive(Default, PartialEq)]
pub enum NetStatus {
    #[default]
    Idle,
    Fetching,
    Ready,
    Error(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OnlineEntry {
    pub time: f32,
    pub name: String,
    #[serde(deserialize_with = "deserialize_seed", serialize_with = "serialize_seed")]
    pub seed: u64,
    pub id: String,
    #[serde(default)]
    pub level: Option<String>,
}

fn serialize_seed<S: serde::Serializer>(seed: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&seed.to_string())
}

fn deserialize_seed<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

// Resource-based signaling (instead of events)

#[derive(Resource, Default)]
pub struct PendingSubmission(pub Option<SubmissionData>);

pub struct SubmissionData {
    pub time: f32,
    pub seed: u64,
    pub inputs: Vec<FrameInput>,
    pub level: String,
}

#[derive(Resource, Default)]
pub struct PendingReplayFetch(pub Option<usize>);

#[derive(Resource, Default)]
pub struct ReplayFetchStatus {
    pub loading: bool,
}

// --- Plugin ---

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineLeaderboard>()
            .init_resource::<ReplayFetchStatus>()
            .init_resource::<PendingSubmission>()
            .init_resource::<PendingReplayFetch>();

        #[cfg(not(target_family = "wasm"))]
        {
            app.insert_resource(RefreshTimer(Timer::from_seconds(10.0, TimerMode::Repeating)))
                .add_systems(OnEnter(GamePhase::TitleScreen), trigger_fetch_leaderboard)
                .add_systems(OnEnter(GamePhase::Generating), trigger_fetch_leaderboard)
                .add_systems(
                    Update,
                    (
                        poll_leaderboard_response,
                        handle_submit_score,
                        poll_submit_response,
                        handle_fetch_replay,
                        poll_replay_response,
                        periodic_refresh,
                    ),
                );
        }
    }
}

// --- Native networking (not available on WASM) ---

#[cfg(not(target_family = "wasm"))]
mod native {
    use super::*;
    use std::sync::{mpsc, Mutex};

    #[derive(Resource)]
    pub(super) struct LeaderboardReceiver(pub Mutex<mpsc::Receiver<Result<Vec<OnlineEntry>, String>>>);

    #[derive(Resource)]
    pub(super) struct SubmitReceiver(pub Mutex<mpsc::Receiver<Result<(), String>>>);

    #[derive(Resource)]
    pub(super) struct ReplayReceiver(pub Mutex<mpsc::Receiver<Result<ReplayPayload, String>>>);

    #[derive(Deserialize)]
    pub(super) struct ReplayPayload {
        #[serde(deserialize_with = "super::deserialize_seed")]
        pub seed: u64,
        pub inputs: Vec<FrameInput>,
        #[serde(default)]
        pub level: Option<String>,
    }

    #[derive(Deserialize)]
    pub(super) struct SubmitResponse {
        pub ok: bool,
    }
}

#[cfg(not(target_family = "wasm"))]
use native::*;

#[cfg(not(target_family = "wasm"))]
#[derive(Resource)]
struct RefreshTimer(Timer);

// --- Leaderboard fetch ---

#[cfg(not(target_family = "wasm"))]
fn trigger_fetch_leaderboard(
    mut commands: Commands,
    mut leaderboard: ResMut<OnlineLeaderboard>,
    game_mode: Res<GameMode>,
    current_level: Res<CurrentLevel>,
) {
    if *game_mode == GameMode::Zen {
        leaderboard.entries.clear();
        leaderboard.status = NetStatus::Idle;
        return;
    }
    leaderboard.status = NetStatus::Fetching;
    let (tx, rx) = std::sync::mpsc::channel();
    commands.insert_resource(LeaderboardReceiver(std::sync::Mutex::new(rx)));

    let level = current_level.name().to_string();
    std::thread::spawn(move || {
        let result = fetch_leaderboard_http(&level);
        let _ = tx.send(result);
    });
}

#[cfg(not(target_family = "wasm"))]
fn periodic_refresh(
    mut commands: Commands,
    mut leaderboard: ResMut<OnlineLeaderboard>,
    mut timer: ResMut<RefreshTimer>,
    time: Res<Time>,
    game_mode: Res<GameMode>,
    current_level: Res<CurrentLevel>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() && leaderboard.status != NetStatus::Fetching {
        if *game_mode == GameMode::Zen {
            return;
        }
        leaderboard.status = NetStatus::Fetching;
        let (tx, rx) = std::sync::mpsc::channel();
        commands.insert_resource(LeaderboardReceiver(std::sync::Mutex::new(rx)));
        let level = current_level.name().to_string();
        std::thread::spawn(move || {
            let result = fetch_leaderboard_http(&level);
            let _ = tx.send(result);
        });
    }
}

#[cfg(not(target_family = "wasm"))]
fn fetch_leaderboard_http(level: &str) -> Result<Vec<OnlineEntry>, String> {
    let url = format!("{API_URL}/leaderboard?level={level}");
    let body: Vec<OnlineEntry> = ureq::get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(body)
}

#[cfg(not(target_family = "wasm"))]
fn poll_leaderboard_response(
    mut leaderboard: ResMut<OnlineLeaderboard>,
    receiver: Option<Res<LeaderboardReceiver>>,
) {
    let Some(receiver) = receiver else { return };
    let rx = receiver.0.lock().unwrap();
    match rx.try_recv() {
        Ok(Ok(entries)) => {
            leaderboard.entries = entries;
            leaderboard.status = NetStatus::Ready;
        }
        Ok(Err(e)) => {
            warn!("Leaderboard fetch failed: {e}");
            leaderboard.status = NetStatus::Error(e);
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            if leaderboard.status == NetStatus::Fetching {
                leaderboard.status = NetStatus::Error("Connection lost".into());
            }
        }
    }
}

// --- Score submission ---

#[cfg(not(target_family = "wasm"))]
fn handle_submit_score(
    mut commands: Commands,
    mut pending: ResMut<PendingSubmission>,
    player_name: Option<Res<PlayerName>>,
) {
    let Some(data) = pending.0.take() else { return };
    let name = player_name
        .as_ref()
        .map(|n| n.0.clone())
        .unwrap_or_else(|| "Anonymous".into());

    let (tx, rx) = std::sync::mpsc::channel();
    commands.insert_resource(SubmitReceiver(std::sync::Mutex::new(rx)));

    std::thread::spawn(move || {
        let result = submit_score_http(data.time, &name, data.seed, &data.inputs, &data.level);
        let _ = tx.send(result);
    });
}

#[cfg(not(target_family = "wasm"))]
fn submit_score_http(
    time: f32,
    name: &str,
    seed: u64,
    inputs: &[FrameInput],
    level: &str,
) -> Result<(), String> {
    let url = format!("{API_URL}/leaderboard");
    let body = serde_json::json!({
        "time": time,
        "name": name,
        "seed": seed.to_string(),
        "inputs": inputs,
        "level": level,
    });
    let resp: SubmitResponse = ureq::post(&url)
        .send_json(&body)
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    if resp.ok {
        info!("Score submitted successfully");
    } else {
        info!("Score did not qualify for top 5");
    }
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
fn poll_submit_response(
    mut commands: Commands,
    receiver: Option<Res<SubmitReceiver>>,
    mut leaderboard: ResMut<OnlineLeaderboard>,
    game_mode: Res<GameMode>,
    current_level: Res<CurrentLevel>,
) {
    let Some(receiver) = receiver else { return };
    let rx = receiver.0.lock().unwrap();
    match rx.try_recv() {
        Ok(Ok(())) => {
            // Re-fetch leaderboard after successful submission
            if *game_mode == GameMode::Levels {
                leaderboard.status = NetStatus::Fetching;
                let (tx, rx) = std::sync::mpsc::channel();
                commands.insert_resource(LeaderboardReceiver(std::sync::Mutex::new(rx)));
                let level = current_level.name().to_string();
                std::thread::spawn(move || {
                    let result = fetch_leaderboard_http(&level);
                    let _ = tx.send(result);
                });
            }
        }
        Ok(Err(e)) => {
            warn!("Score submission failed: {e}");
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
    }
}

// --- Replay fetch ---

#[cfg(not(target_family = "wasm"))]
fn handle_fetch_replay(
    mut commands: Commands,
    mut pending: ResMut<PendingReplayFetch>,
    mut status: ResMut<ReplayFetchStatus>,
    current_level: Res<CurrentLevel>,
) {
    let Some(index) = pending.0.take() else { return };
    status.loading = true;
    let level = current_level.name().to_string();

    let (tx, rx) = std::sync::mpsc::channel();
    commands.insert_resource(ReplayReceiver(std::sync::Mutex::new(rx)));

    std::thread::spawn(move || {
        let result = fetch_replay_http(index, &level);
        let _ = tx.send(result);
    });
}

#[cfg(not(target_family = "wasm"))]
fn fetch_replay_http(index: usize, level: &str) -> Result<ReplayPayload, String> {
    let url = format!("{API_URL}/replay/{index}?level={level}");
    let payload: ReplayPayload = ureq::get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(payload)
}

#[cfg(not(target_family = "wasm"))]
fn poll_replay_response(
    receiver: Option<Res<ReplayReceiver>>,
    mut replay_data: ResMut<ReplayData>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut status: ResMut<ReplayFetchStatus>,
) {
    let Some(receiver) = receiver else { return };
    let rx = receiver.0.lock().unwrap();
    match rx.try_recv() {
        Ok(Ok(payload)) => {
            replay_data.frames = payload.inputs;
            replay_data.seed = payload.seed;
            replay_data.frame_index = 0;
            status.loading = false;
            next_state.set(GamePhase::Replaying);
        }
        Ok(Err(e)) => {
            warn!("Replay fetch failed: {e}");
            status.loading = false;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            if status.loading {
                status.loading = false;
                warn!("Replay fetch connection lost");
            }
        }
    }
}
