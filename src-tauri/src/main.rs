#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod calendar;
mod tasks;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedData {
    events: Vec<calendar::Event>,
    tasks: Vec<tasks::Task>,
}

struct AppState {
    cache: Mutex<Option<CachedData>>,
}

fn get_base_dir() -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_default();
    exe_path.parent().unwrap_or(&exe_path).to_path_buf()
}

fn get_cache_path() -> PathBuf {
    get_base_dir().join("cache.json")
}

fn load_cache() -> Option<CachedData> {
    let path = get_cache_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            return serde_json::from_str(&content).ok();
        }
    }
    None
}

fn save_cache(data: &CachedData) {
    let path = get_cache_path();
    if let Ok(content) = serde_json::to_string_pretty(data) {
        let _ = fs::write(&path, content);
    }
}

#[tauri::command]
async fn get_data(state: tauri::State<'_, AppState>) -> Result<CachedData, String> {
    // Try to fetch fresh data
    let events_result = calendar::get_events(60).await;
    let tasks_result = tasks::get_tasks().await;

    match (events_result, tasks_result) {
        (Ok(events), Ok(tasks)) => {
            let data = CachedData { events, tasks };

            // Update cache
            save_cache(&data);
            *state.cache.lock().unwrap() = Some(data.clone());

            Ok(data)
        }
        (Err(e), _) | (_, Err(e)) => {
            // Return cached data on error
            if let Some(cached) = state.cache.lock().unwrap().clone() {
                Ok(cached)
            } else {
                Err(e)
            }
        }
    }
}

#[tauri::command]
async fn get_cached_data(state: tauri::State<'_, AppState>) -> Result<CachedData, String> {
    if let Some(cached) = state.cache.lock().unwrap().clone() {
        Ok(cached)
    } else {
        Ok(CachedData {
            events: vec![],
            tasks: vec![],
        })
    }
}

#[tauri::command]
async fn complete_task(task_id: String, tasklist_id: String) -> Result<bool, String> {
    tasks::complete_task(&task_id, &tasklist_id).await
}

#[tauri::command]
async fn close_widget(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
async fn start_drag(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

fn main() {
    // Load cached data at startup
    let cached = load_cache();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            cache: Mutex::new(cached),
        })
        .invoke_handler(tauri::generate_handler![
            get_data,
            get_cached_data,
            complete_task,
            close_widget,
            start_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
