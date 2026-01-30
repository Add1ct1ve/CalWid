use serde::{Deserialize, Serialize};

use crate::auth::get_access_token;

const TASKS_API_BASE: &str = "https://tasks.googleapis.com/tasks/v1";

// Only show tasks from these lists
const ALLOWED_LISTS: &[&str] = &["I dag", "Min huskeliste"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub tasklist_id: String,
}

#[derive(Debug, Deserialize)]
struct TaskListsResponse {
    items: Option<Vec<TaskListEntry>>,
}

#[derive(Debug, Deserialize)]
struct TaskListEntry {
    id: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TasksResponse {
    items: Option<Vec<TaskEntry>>,
}

#[derive(Debug, Deserialize)]
struct TaskEntry {
    id: Option<String>,
    title: Option<String>,
    status: Option<String>,
}

pub async fn get_tasks() -> Result<Vec<Task>, String> {
    let access_token = get_access_token().await?;
    let client = reqwest::Client::new();

    // Get all task lists
    let url = format!("{}/users/@me/lists", TASKS_API_BASE);
    let response = client
        .get(&url)
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch task lists: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Tasks API error: {}", error));
    }

    let tasklists: TaskListsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse task lists: {}", e))?;

    let mut all_tasks = Vec::new();

    if let Some(lists) = tasklists.items {
        for list in lists {
            // Only process allowed lists
            let list_title = list.title.as_deref().unwrap_or("");
            if !ALLOWED_LISTS.contains(&list_title) {
                continue;
            }

            // Get tasks from this list
            let url = format!(
                "{}/lists/{}/tasks?showCompleted=false&maxResults=50",
                TASKS_API_BASE,
                urlencoding::encode(&list.id)
            );

            let response = match client
                .get(&url)
                .bearer_auth(&access_token)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to fetch tasks from {}: {}", list_title, e);
                    continue;
                }
            };

            if !response.status().is_success() {
                continue;
            }

            let tasks_response: TasksResponse = match response.json().await {
                Ok(t) => t,
                Err(_) => continue,
            };

            if let Some(items) = tasks_response.items {
                for item in items {
                    let title = item.title.unwrap_or_default();
                    if title.is_empty() {
                        continue;
                    }

                    all_tasks.push(Task {
                        id: item.id.unwrap_or_default(),
                        title,
                        completed: item.status.as_deref() == Some("completed"),
                        tasklist_id: list.id.clone(),
                    });
                }
            }
        }
    }

    // Filter out completed tasks and sort
    all_tasks.retain(|t| !t.completed);

    Ok(all_tasks)
}

pub async fn complete_task(task_id: &str, tasklist_id: &str) -> Result<bool, String> {
    let access_token = get_access_token().await?;
    let client = reqwest::Client::new();

    let url = format!(
        "{}/lists/{}/tasks/{}",
        TASKS_API_BASE,
        urlencoding::encode(tasklist_id),
        urlencoding::encode(task_id)
    );

    let body = serde_json::json!({
        "status": "completed"
    });

    let response = client
        .patch(&url)
        .bearer_auth(&access_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to complete task: {}", e))?;

    Ok(response.status().is_success())
}
