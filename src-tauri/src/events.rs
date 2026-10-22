use crate::config;
use crate::model::{now_unix_ms, EventSeverity, HealthEvent};
use std::path::PathBuf;

const MAX_EVENTS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct EventSignal {
    pub source: String,
    pub source_id: String,
    pub title: String,
    pub level: SignalLevel,
    pub message: String,
    pub reason: Option<String>,
}

impl EventSignal {
    fn severity(&self) -> Option<EventSeverity> {
        match self.level {
            SignalLevel::Normal => None,
            SignalLevel::Warning => Some(EventSeverity::Warning),
            SignalLevel::Critical => Some(EventSeverity::Critical),
        }
    }
}

#[derive(Debug, Default)]
pub struct EventTracker {
    events: Vec<HealthEvent>,
}

impl EventTracker {
    pub fn new(events: Vec<HealthEvent>) -> Self {
        let mut tracker = Self { events };
        tracker.normalize();
        tracker
    }

    pub fn snapshot(&self) -> Vec<HealthEvent> {
        self.events.clone()
    }

    pub fn observe(&mut self, signals: Vec<EventSignal>) -> bool {
        let now = now_unix_ms();
        let mut changed = false;
        for signal in signals {
            let open = self.events.iter().position(|event| {
                event.ended_at.is_none()
                    && event.source == signal.source
                    && event.source_id == signal.source_id
            });
            match (signal.severity(), open) {
                (None, Some(index)) => {
                    self.events[index].ended_at = Some(now);
                    changed = true;
                }
                (Some(severity), Some(index)) => {
                    let event = &mut self.events[index];
                    if event.severity != severity
                        || event.title != signal.title
                        || event.message != signal.message
                        || event.reason != signal.reason
                    {
                        event.severity = severity;
                        event.title = signal.title;
                        event.message = signal.message;
                        event.reason = signal.reason;
                        changed = true;
                    }
                }
                (Some(severity), None) => {
                    self.events.insert(
                        0,
                        HealthEvent {
                            id: format!("{}:{}:{now}", signal.source, signal.source_id),
                            source: signal.source,
                            source_id: signal.source_id,
                            title: signal.title,
                            severity,
                            started_at: now,
                            ended_at: None,
                            message: signal.message,
                            reason: signal.reason,
                        },
                    );
                    changed = true;
                }
                (None, None) => {}
            }
        }
        if changed {
            self.normalize();
        }
        changed
    }

    fn normalize(&mut self) {
        self.events.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        self.events.truncate(MAX_EVENTS);
    }
}

fn events_path() -> PathBuf {
    config::config_dir().join("events.json")
}

pub fn load() -> Vec<HealthEvent> {
    let path = events_path();
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<HealthEvent>>(&text).unwrap_or_default()
}

pub fn save(events: &[HealthEvent]) -> anyhow::Result<()> {
    let dir = config::config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = events_path();
    let tmp = dir.join("events.json.tmp");
    let json = serde_json::to_string_pretty(events)?;
    std::fs::write(&tmp, json)?;
    config::replace_atomic(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(level: SignalLevel) -> EventSignal {
        EventSignal {
            source: "service".into(),
            source_id: "media".into(),
            title: "媒体服务".into(),
            level,
            message: "连接失败".into(),
            reason: Some("连接超时".into()),
        }
    }

    #[test]
    fn opens_updates_and_resolves_one_incident() {
        let mut tracker = EventTracker::default();
        assert!(tracker.observe(vec![signal(SignalLevel::Warning)]));
        assert_eq!(tracker.events.len(), 1);
        let started = tracker.events[0].started_at;
        assert!(tracker.observe(vec![signal(SignalLevel::Critical)]));
        assert_eq!(tracker.events.len(), 1);
        assert_eq!(tracker.events[0].started_at, started);
        assert_eq!(tracker.events[0].severity, EventSeverity::Critical);
        assert!(tracker.observe(vec![signal(SignalLevel::Normal)]));
        assert!(tracker.events[0].ended_at.is_some());
    }

    #[test]
    fn repeated_state_does_not_append_events() {
        let mut tracker = EventTracker::default();
        assert!(tracker.observe(vec![signal(SignalLevel::Critical)]));
        assert!(!tracker.observe(vec![signal(SignalLevel::Critical)]));
        assert_eq!(tracker.events.len(), 1);
    }

    #[test]
    fn caps_history() {
        let mut tracker = EventTracker::default();
        for index in 0..(MAX_EVENTS + 12) {
            let mut current = signal(SignalLevel::Critical);
            current.source_id = format!("service-{index}");
            tracker.observe(vec![current]);
        }
        assert_eq!(tracker.events.len(), MAX_EVENTS);
    }
}
