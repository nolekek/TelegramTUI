use std::time::Duration;
use crossterm::event::{Event as CrosstermEvent, EventStream};
use futures::StreamExt;
use tg_core::CoreEvent;
use tokio::sync::broadcast;

#[derive(Debug)]
pub enum AppEvent {
    Terminal(CrosstermEvent),
    Core(CoreEvent),
    Tick,
}

pub struct EventHandler {
    term_stream: EventStream,
    core_rx: broadcast::Receiver<CoreEvent>,
    tick_interval: tokio::time::Interval,
}

impl EventHandler {
    pub fn new(core_rx: broadcast::Receiver<CoreEvent>, tick_rate: Duration) -> Self {
        Self {
            term_stream: EventStream::new(),
            core_rx,
            tick_interval: tokio::time::interval(tick_rate),
        }
    }

    /// Retrieve the next unified event across terminal input and core broadcast stream.
    pub async fn next(&mut self) -> Option<AppEvent> {
        tokio::select! {
            maybe_term = self.term_stream.next() => {
                match maybe_term {
                    Some(Ok(evt)) => Some(AppEvent::Terminal(evt)),
                    _ => None,
                }
            }
            core_res = self.core_rx.recv() => {
                match core_res {
                    Ok(evt) => Some(AppEvent::Core(evt)),
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("TUI core event receiver lagged by {} events; continuing", skipped);
                        Some(AppEvent::Tick)
                    }
                    Err(broadcast::error::RecvError::Closed) => None,
                }
            }
            _ = self.tick_interval.tick() => {
                Some(AppEvent::Tick)
            }
        }
    }
}
