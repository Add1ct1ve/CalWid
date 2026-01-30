use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration, Datelike, Weekday, NaiveDate};

use crate::auth::get_access_token;

const CALENDAR_API_BASE: &str = "https://www.googleapis.com/calendar/v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub color: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub date: String,
    pub time: String,
    pub time_range: String,
    pub date_formatted: String,
    pub color: String,
    pub calendar: String,
    pub location: String,
    pub description: String,
    pub is_all_day: bool,
}

#[derive(Debug, Deserialize)]
struct CalendarListResponse {
    items: Option<Vec<CalendarListEntry>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CalendarListEntry {
    id: String,
    summary: Option<String>,
    #[serde(rename = "backgroundColor")]
    background_color: Option<String>,
    primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct EventsListResponse {
    items: Option<Vec<EventEntry>>,
}

#[derive(Debug, Deserialize)]
struct EventEntry {
    id: Option<String>,
    summary: Option<String>,
    start: Option<EventDateTime>,
    end: Option<EventDateTime>,
    location: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventDateTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
    date: Option<String>,
}

pub async fn get_calendars() -> Result<Vec<Calendar>, String> {
    let access_token = get_access_token().await?;
    let client = reqwest::Client::new();

    let mut calendars = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut url = format!("{}/users/me/calendarList", CALENDAR_API_BASE);
        if let Some(ref token) = page_token {
            url.push_str(&format!("?pageToken={}", token));
        }

        let response = client
            .get(&url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch calendars: {}", e))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Calendar API error: {}", error));
        }

        let data: CalendarListResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse calendar list: {}", e))?;

        if let Some(items) = data.items {
            for item in items {
                calendars.push(Calendar {
                    id: item.id,
                    name: item.summary.unwrap_or_else(|| "Unnamed".to_string()),
                    color: item.background_color.unwrap_or_else(|| "#3b82f6".to_string()),
                    primary: item.primary.unwrap_or(false),
                });
            }
        }

        page_token = data.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    Ok(calendars)
}

pub async fn get_events(days: i32) -> Result<Vec<Event>, String> {
    let access_token = get_access_token().await?;
    let client = reqwest::Client::new();

    let calendars = get_calendars().await?;

    // Start from beginning of current week (Monday)
    let now = Utc::now();
    let days_since_monday = now.weekday().num_days_from_monday() as i64;
    let start_of_week = now - Duration::days(days_since_monday);
    let start_of_week = start_of_week.format("%Y-%m-%dT00:00:00Z").to_string();

    let time_max = (now + Duration::days(days as i64)).format("%Y-%m-%dT23:59:59Z").to_string();

    let mut all_events = Vec::new();

    for calendar in calendars {
        let url = format!(
            "{}/calendars/{}/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime",
            CALENDAR_API_BASE,
            urlencoding::encode(&calendar.id),
            urlencoding::encode(&start_of_week),
            urlencoding::encode(&time_max)
        );

        let response = match client
            .get(&url)
            .bearer_auth(&access_token)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to fetch events from {}: {}", calendar.name, e);
                continue;
            }
        };

        if !response.status().is_success() {
            eprintln!("Error fetching events from {}: {}", calendar.name, response.status());
            continue;
        }

        let data: EventsListResponse = match response.json().await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to parse events from {}: {}", calendar.name, e);
                continue;
            }
        };

        if let Some(items) = data.items {
            for item in items {
                let (date, time, time_range, date_formatted, is_all_day) = parse_event_time(&item);

                all_events.push(Event {
                    id: item.id.unwrap_or_default(),
                    title: item.summary.unwrap_or_else(|| "(No title)".to_string()),
                    date,
                    time,
                    time_range,
                    date_formatted,
                    color: calendar.color.clone(),
                    calendar: calendar.name.clone(),
                    location: item.location.unwrap_or_default(),
                    description: item.description.unwrap_or_default(),
                    is_all_day,
                });
            }
        }
    }

    // Sort by date and time
    all_events.sort_by(|a, b| {
        let date_cmp = a.date.cmp(&b.date);
        if date_cmp != std::cmp::Ordering::Equal {
            return date_cmp;
        }
        // All-day events come first
        if a.is_all_day && !b.is_all_day {
            return std::cmp::Ordering::Less;
        }
        if !a.is_all_day && b.is_all_day {
            return std::cmp::Ordering::Greater;
        }
        a.time.cmp(&b.time)
    });

    Ok(all_events)
}

fn parse_event_time(event: &EventEntry) -> (String, String, String, String, bool) {
    let start = event.start.as_ref();
    let end = event.end.as_ref();

    if let Some(start) = start {
        if let Some(date) = &start.date {
            // All-day event
            let date_formatted = format_date_string(date);
            return (date.clone(), "All day".to_string(), "All day".to_string(), date_formatted, true);
        }

        if let Some(dt_str) = &start.date_time {
            // Timed event
            if let Ok(dt) = DateTime::parse_from_rfc3339(dt_str) {
                let local = dt.with_timezone(&chrono::Local);
                let date = local.format("%Y-%m-%d").to_string();
                let time = local.format("%H:%M").to_string();
                let date_formatted = local.format("%A, %d. %B").to_string();

                let time_range = if let Some(end) = end {
                    if let Some(end_dt_str) = &end.date_time {
                        if let Ok(end_dt) = DateTime::parse_from_rfc3339(end_dt_str) {
                            let end_local = end_dt.with_timezone(&chrono::Local);
                            format!("{} - {}", local.format("%H:%M"), end_local.format("%H:%M"))
                        } else {
                            time.clone()
                        }
                    } else {
                        time.clone()
                    }
                } else {
                    time.clone()
                };

                return (date, time, time_range, date_formatted, false);
            }
        }
    }

    // Fallback
    let now = Utc::now();
    let date = now.format("%Y-%m-%d").to_string();
    (date, "All day".to_string(), "All day".to_string(), "Unknown".to_string(), true)
}

fn format_date_string(date_str: &str) -> String {
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        date.format("%A, %d. %B").to_string()
    } else {
        date_str.to_string()
    }
}
