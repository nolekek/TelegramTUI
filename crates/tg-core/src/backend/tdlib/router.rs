use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::error::{CoreError, Result};

pub type RequestResult = Result<Value>;

/// Correlates asynchronous outgoing TDLib requests with incoming responses
/// multiplexed over the `td_receive` stream.
#[derive(Debug)]
pub struct RequestRouter {
    counter: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<RequestResult>>>,
    default_timeout: Duration,
}

impl Default for RequestRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestRouter {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Creates a new RequestRouter with the default 30-second timeout.
    pub fn new() -> Self {
        Self::with_timeout(Self::DEFAULT_TIMEOUT)
    }

    /// Creates a new RequestRouter with a custom default timeout.
    pub fn with_timeout(default_timeout: Duration) -> Self {
        Self {
            counter: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            default_timeout,
        }
    }

    /// Returns the default timeout duration.
    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// Registers a new pending request, returning the generated `@extra` ID
    /// and the oneshot Receiver for the eventual response.
    pub fn register(&self) -> (String, oneshot::Receiver<RequestResult>) {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let extra_id = format!("req-{}", id);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock();
            pending.insert(extra_id.clone(), tx);
        }
        (extra_id, rx)
    }

    /// Cancels a pending request by removing it from the correlation map.
    /// Returns true if the request was present and removed.
    pub fn cancel(&self, extra_id: &str) -> bool {
        let mut pending = self.pending.lock();
        pending.remove(extra_id).is_some()
    }

    /// Dispatches an incoming TDLib JSON value.
    ///
    /// - If `@extra` is present and matches a pending request, the sender is triggered
    ///   and `None` is returned (consuming the value).
    /// - If `@extra` is present but unknown/expired, a warning is logged and `None` is returned.
    /// - If `@extra` is absent, the value is returned unchanged as `Some(val)` for update handling.
    pub fn dispatch(&self, val: Value) -> Option<Value> {
        let extra_opt = val.get("@extra").and_then(|e| match e {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });

        if let Some(extra_id) = extra_opt {
            let sender = {
                let mut pending = self.pending.lock();
                pending.remove(&extra_id)
            };

            if let Some(tx) = sender {
                let is_error = val.get("@type").and_then(|t| t.as_str()) == Some("error");
                let res = if is_error {
                    let code = val.get("code").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
                    let message = val
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown TDLib error")
                        .to_string();
                    Err(CoreError::RpcError { code, message })
                } else {
                    Ok(val)
                };
                let _ = tx.send(res);
            } else {
                tracing::debug!(
                    "Received response for unknown or cancelled request @extra={}",
                    extra_id
                );
            }
            None
        } else {
            Some(val)
        }
    }

    /// Aborts all in-flight requests, notifying all waiting receivers with an error.
    /// Used when the TDLib backend shuts down or encounters a fatal crash.
    pub fn abort_all(&self, reason: &str) {
        let mut pending = self.pending.lock();
        for (extra_id, tx) in pending.drain() {
            let _ = tx.send(Err(CoreError::Internal(format!(
                "Request {} aborted: {}",
                extra_id, reason
            ))));
        }
    }

    /// Returns the number of currently in-flight requests.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }
}

/// RAII guard that automatically deregisters a request if dropped before completion.
pub struct RequestRegistration<'a> {
    router: &'a RequestRouter,
    extra_id: String,
    completed: bool,
}

impl<'a> RequestRegistration<'a> {
    pub fn new(router: &'a RequestRouter, extra_id: String) -> Self {
        Self {
            router,
            extra_id,
            completed: false,
        }
    }

    pub fn extra_id(&self) -> &str {
        &self.extra_id
    }

    pub fn complete(mut self) {
        self.completed = true;
    }
}

impl<'a> Drop for RequestRegistration<'a> {
    fn drop(&mut self) {
        if !self.completed {
            self.router.cancel(&self.extra_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_router_register_and_dispatch_success() {
        let router = RequestRouter::new();
        let (extra_id, mut rx) = router.register();
        assert_eq!(router.pending_count(), 1);

        let incoming = json!({
            "@type": "ok",
            "@extra": extra_id
        });

        let unhandled = router.dispatch(incoming);
        assert!(unhandled.is_none());
        assert_eq!(router.pending_count(), 0);

        let res = rx.try_recv().expect("Expected response on oneshot");
        assert!(res.is_ok());
        let val = res.unwrap();
        assert_eq!(val.get("@type").and_then(|t| t.as_str()), Some("ok"));
    }

    #[test]
    fn test_router_dispatch_rpc_error() {
        let router = RequestRouter::new();
        let (extra_id, mut rx) = router.register();

        let incoming_err = json!({
            "@type": "error",
            "code": 400,
            "message": "PHONE_CODE_INVALID",
            "@extra": extra_id
        });

        let unhandled = router.dispatch(incoming_err);
        assert!(unhandled.is_none());

        let res = rx.try_recv().expect("Expected response on oneshot");
        assert!(res.is_err());
        match res.unwrap_err() {
            CoreError::RpcError { code, message } => {
                assert_eq!(code, 400);
                assert_eq!(message, "PHONE_CODE_INVALID");
            }
            other => panic!("Expected RpcError, got {:?}", other),
        }
    }

    #[test]
    fn test_router_unsolicited_update_passthrough() {
        let router = RequestRouter::new();
        let spontaneous = json!({
            "@type": "updateNewMessage",
            "message": { "id": 1 }
        });

        let unhandled = router.dispatch(spontaneous.clone());
        assert_eq!(unhandled, Some(spontaneous));
    }

    #[test]
    fn test_router_cancel_and_drop_guard() {
        let router = RequestRouter::new();
        {
            let (extra_id, _rx) = router.register();
            assert_eq!(router.pending_count(), 1);
            let _guard = RequestRegistration::new(&router, extra_id);
            // guard dropped here without complete()
        }
        assert_eq!(router.pending_count(), 0);
    }

    #[test]
    fn test_router_abort_all() {
        let router = RequestRouter::new();
        let (_id1, mut rx1) = router.register();
        let (_id2, mut rx2) = router.register();
        assert_eq!(router.pending_count(), 2);

        router.abort_all("Engine shutting down");
        assert_eq!(router.pending_count(), 0);

        let res1 = rx1.try_recv().unwrap();
        assert!(res1.is_err());
        let res2 = rx2.try_recv().unwrap();
        assert!(res2.is_err());
    }
}
