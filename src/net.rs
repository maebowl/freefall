use std::sync::{mpsc, Mutex};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::level::GamePhase;
use crate::replay::{FrameInput, ReplayData};

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
    pub level: u32,
    pub id: String,
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
    pub level: u32,
    pub inputs: Vec<FrameInput>,
}

#[derive(Resource, Default)]
pub struct PendingReplayFetch(pub Option<usize>);

// --- Internal channel resources (Mutex for Sync) ---

#[derive(Resource)]
struct LeaderboardReceiver(Mutex<mpsc::Receiver<Result<Vec<OnlineEntry>, String>>>);

#[derive(Resource)]
struct SubmitReceiver(Mutex<mpsc::Receiver<Result<(), String>>>);

#[derive(Resource)]
struct ReplayReceiver(Mutex<mpsc::Receiver<Result<ReplayPayload, String>>>);

#[derive(Resource, Default)]
pub struct ReplayFetchStatus {
    pub loading: bool,
}

#[derive(Deserialize)]
struct ReplayPayload {
    #[serde(deserialize_with = "deserialize_seed")]
    seed: u64,
    level: u32,
    inputs: Vec<FrameInput>,
}

#[derive(Deserialize)]
struct SubmitResponse {
    ok: bool,
}

// --- Plugin ---

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineLeaderboard>()
            .init_resource::<ReplayFetchStatus>()
            .init_resource::<PendingSubmission>()
            .init_resource::<PendingReplayFetch>()
            .add_systems(OnEnter(GamePhase::TitleScreen), trigger_fetch_leaderboard)
            .add_systems(
                Update,
                (
                    poll_leaderboard_response,
                    handle_submit_score,
                    poll_submit_response,
                    handle_fetch_replay,
                    poll_replay_response,
                ),
            );
    }
}

// --- Leaderboard fetch ---

fn trigger_fetch_leaderboard(
    mut commands: Commands,
    mut leaderboard: ResMut<OnlineLeaderboard>,
) {
    leaderboard.status = NetStatus::Fetching;
    let (tx, rx) = mpsc::channel();
    commands.insert_resource(LeaderboardReceiver(Mutex::new(rx)));

    std::thread::spawn(move || {
        let result = fetch_leaderboard_http();
        let _ = tx.send(result);
    });
}

fn fetch_leaderboard_http() -> Result<Vec<OnlineEntry>, String> {
    let url = format!("{API_URL}/leaderboard");
    let body: Vec<OnlineEntry> = ureq::get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(body)
}

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
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            if leaderboard.status == NetStatus::Fetching {
                leaderboard.status = NetStatus::Error("Connection lost".into());
            }
        }
    }
}

// --- Score submission ---

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

    let (tx, rx) = mpsc::channel();
    commands.insert_resource(SubmitReceiver(Mutex::new(rx)));

    std::thread::spawn(move || {
        let result = submit_score_http(data.time, &name, data.seed, data.level, &data.inputs);
        let _ = tx.send(result);
    });
}

fn submit_score_http(
    time: f32,
    name: &str,
    seed: u64,
    level: u32,
    inputs: &[FrameInput],
) -> Result<(), String> {
    let url = format!("{API_URL}/leaderboard");
    let body = serde_json::json!({
        "time": time,
        "name": name,
        "seed": seed.to_string(),
        "level": level,
        "inputs": inputs,
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

fn poll_submit_response(
    mut commands: Commands,
    receiver: Option<Res<SubmitReceiver>>,
    mut leaderboard: ResMut<OnlineLeaderboard>,
) {
    let Some(receiver) = receiver else { return };
    let rx = receiver.0.lock().unwrap();
    match rx.try_recv() {
        Ok(Ok(())) => {
            // Re-fetch leaderboard after successful submission
            leaderboard.status = NetStatus::Fetching;
            let (tx, rx) = mpsc::channel();
            commands.insert_resource(LeaderboardReceiver(Mutex::new(rx)));
            std::thread::spawn(move || {
                let result = fetch_leaderboard_http();
                let _ = tx.send(result);
            });
        }
        Ok(Err(e)) => {
            warn!("Score submission failed: {e}");
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {}
    }
}

// --- Replay fetch ---

fn handle_fetch_replay(
    mut commands: Commands,
    mut pending: ResMut<PendingReplayFetch>,
    mut status: ResMut<ReplayFetchStatus>,
) {
    let Some(index) = pending.0.take() else { return };
    status.loading = true;

    let (tx, rx) = mpsc::channel();
    commands.insert_resource(ReplayReceiver(Mutex::new(rx)));

    std::thread::spawn(move || {
        let result = fetch_replay_http(index);
        let _ = tx.send(result);
    });
}

fn fetch_replay_http(index: usize) -> Result<ReplayPayload, String> {
    let url = format!("{API_URL}/replay/{index}");
    let payload: ReplayPayload = ureq::get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(payload)
}

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
            replay_data.level = payload.level;
            replay_data.frame_index = 0;
            status.loading = false;
            next_state.set(GamePhase::Replaying);
        }
        Ok(Err(e)) => {
            warn!("Replay fetch failed: {e}");
            status.loading = false;
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            if status.loading {
                status.loading = false;
                warn!("Replay fetch connection lost");
            }
        }
    }
}
