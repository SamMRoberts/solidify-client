//! Narrow desktop DTOs. Protocol types and their APIs remain serialization-free.
use serde::Serialize;
use solidify_client::{
    application::{Application, Phase},
    protocols::presentation::{PresentationEvent, TextColor, TextControl, TextStyle},
};
use std::sync::Arc;
use tauri::{State, WebviewWindow};

fn authorize(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("This window cannot control connections.".into());
    }
    let url = window
        .url()
        .map_err(|_| "Cannot verify the application origin.")?;
    if allowed_origin(window.label(), &url) {
        Ok(())
    } else {
        Err("Only application content can control connections.".into())
    }
}
fn allowed_origin(label: &str, url: &tauri::Url) -> bool {
    let bundled = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"));
    let development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("localhost")
        && url.port() == Some(1420);
    label == "main" && (bundled || development)
}

fn id(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| "Invalid connection identity.".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    id: String,
    phase: &'static str,
    message: String,
    events: Vec<Event>,
    finished: bool,
    remote_echo: bool,
    masking_generation: String,
}
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    Text { text: String },
    Style { style: Style },
    Control { control: &'static str },
}
#[derive(Serialize)]
struct Style {
    foreground: &'static str,
    background: &'static str,
    bold: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}
impl From<TextStyle> for Style {
    fn from(s: TextStyle) -> Self {
        Self {
            foreground: color(s.foreground),
            background: color(s.background),
            bold: s.bold,
            italic: s.italic,
            underline: s.underline,
            inverse: s.inverse,
        }
    }
}
fn color(color: TextColor) -> &'static str {
    match color {
        TextColor::Default => "default",
        TextColor::Black => "black",
        TextColor::Red => "red",
        TextColor::Green => "green",
        TextColor::Yellow => "yellow",
        TextColor::Blue => "blue",
        TextColor::Magenta => "magenta",
        TextColor::Cyan => "cyan",
        TextColor::White => "white",
    }
}
fn control_name(control: TextControl) -> &'static str {
    match control {
        TextControl::CarriageReturn => "CarriageReturn",
        TextControl::LineFeed => "LineFeed",
        TextControl::Tab => "Tab",
        TextControl::Backspace => "Backspace",
        TextControl::Bell => "Bell",
    }
}
#[tauri::command]
pub async fn start_connection(
    window: WebviewWindow,
    app: State<'_, Arc<Application>>,
    host: String,
    port: u32,
) -> Result<String, String> {
    authorize(&window)?;
    app.start(&host, port)
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn poll_connection(
    window: WebviewWindow,
    app: State<'_, Arc<Application>>,
    session_id: String,
) -> Result<Snapshot, String> {
    authorize(&window)?;
    let poll = app
        .poll(id(&session_id)?)
        .await
        .map_err(|e| e.to_string())?;
    let phase = match poll.status.phase {
        Phase::Resolving => "resolving",
        Phase::Connecting => "connecting",
        Phase::Connected => "connected",
        Phase::Disconnecting => "disconnecting",
        Phase::Closed => "closed",
    };
    let events = poll
        .events
        .into_iter()
        .map(|event| match event {
            PresentationEvent::Text(text) => Event::Text { text },
            PresentationEvent::StyleChanged(style) => Event::Style {
                style: style.into(),
            },
            PresentationEvent::Control(control) => Event::Control {
                control: control_name(control),
            },
        })
        .collect();
    Ok(Snapshot {
        id: poll.id.to_string(),
        phase,
        message: poll.status.message,
        events,
        finished: poll.finished,
        remote_echo: poll.options.remote_echo,
        masking_generation: poll.options.masking_generation.to_string(),
    })
}
#[tauri::command]
pub async fn send_line(
    window: WebviewWindow,
    app: State<'_, Arc<Application>>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    authorize(&window)?;
    app.send_line(id(&session_id)?, &text)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn update_viewport(
    window: WebviewWindow,
    app: State<'_, Arc<Application>>,
    session_id: String,
    columns: u32,
    rows: u32,
) -> Result<(), String> {
    authorize(&window)?;
    app.update_viewport(id(&session_id)?, columns, rows)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn disconnect(
    window: WebviewWindow,
    app: State<'_, Arc<Application>>,
    session_id: String,
) -> Result<(), String> {
    authorize(&window)?;
    app.disconnect(id(&session_id)?)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_require_main_window_and_application_origin() {
        for origin in ["tauri://localhost", "http://tauri.localhost"] {
            let url = tauri::Url::parse(origin).unwrap();
            assert!(allowed_origin("main", &url));
            assert!(!allowed_origin("other", &url));
        }
        for origin in [
            "https://example.invalid",
            "https://localhost:1420",
            "http://localhost:4000",
            "tauri://remote",
        ] {
            assert!(!allowed_origin("main", &tauri::Url::parse(origin).unwrap()));
        }
        assert_eq!(
            allowed_origin("main", &tauri::Url::parse("http://localhost:1420").unwrap()),
            cfg!(debug_assertions)
        );
        assert_eq!(id("18446744073709551615").unwrap(), u64::MAX);
        assert!(id("18446744073709551616").is_err());
    }
}
