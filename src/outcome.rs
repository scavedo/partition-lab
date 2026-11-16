//! What an action did, reported back to the UI: whether it was refused, a
//! short message for the device card, log lines, and messages to animate.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Info,
    Sync,
    Refused,
}

/// One line for the event log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Event {
    pub kind: EventKind,
    pub text: String,
}

/// What an action did, for the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// False if the action was refused.
    pub ok: bool,
    /// Short message to show on the acting device's card.
    pub message: Option<String>,
    pub events: Vec<Event>,
    /// Messages sent between devices, as [from, to] hops, for animation.
    pub transfers: Vec<[usize; 2]>,
}

impl Outcome {
    pub(crate) fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: Some(message.into()),
            ..Self::default()
        }
    }

    pub(crate) fn info(&mut self, text: impl Into<String>) {
        self.events.push(Event {
            kind: EventKind::Info,
            text: text.into(),
        });
    }

    pub(crate) fn sync(&mut self, text: impl Into<String>) {
        self.events.push(Event {
            kind: EventKind::Sync,
            text: text.into(),
        });
    }

    pub(crate) fn refuse(mut self, message: impl Into<String>, log: impl Into<String>) -> Self {
        self.ok = false;
        self.message = Some(message.into());
        self.events.push(Event {
            kind: EventKind::Refused,
            text: log.into(),
        });
        self
    }
}
