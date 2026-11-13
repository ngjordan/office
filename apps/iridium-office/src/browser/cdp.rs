//! The DevTools Protocol client: JSON over a WebSocket.
//!
//! This replaces `headless_chrome`, whose build script pulls in a GPL-3.0
//! code generator. The protocol is simple enough to speak directly, and the
//! Office links only permissive licences.
//!
//! Two things make it more than a JSON wrapper:
//!
//! - **Flat sessions.** `Target.attachToTarget` with `flatten: true` gives a
//!   session id that goes on every message for that page, so a page call is
//!   one message rather than a nested `Target.sendMessageToTarget`.
//! - **One thread owns the socket.** It reads, it writes, and it hands
//!   replies to whoever asked. Callers post a command and wait. Sharing one
//!   socket between a reader and writers does not work: the reader wins
//!   every race for the socket, and the writers never get to send.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tungstenite::client::IntoClientRequest;

/// How long a call waits for its answer before giving up.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the owner waits for a message before it looks for work.
///
/// The owner does everything, so this is also the longest a caller waits to
/// have its command written out.
const READ_TIMEOUT: Duration = Duration::from_millis(20);

// The browser's debugging endpoint is plain `ws://` on the loopback
// interface, so the socket needs no TLS: `WebSocket<TcpStream>`.

/// Something for the socket's owner to do.
enum Command {
    /// Write a request out, and answer the caller when it comes back.
    Call {
        id: u64,
        payload: String,
        reply: Sender<Result<Value, String>>,
    },
    /// Hand this caller the page's events from now on.
    Listen(Sender<Value>),
}

/// A live connection to the browser's DevTools endpoint.
pub struct Cdp {
    /// The socket owner's inbox.
    commands: Sender<Command>,
    /// The next request id.
    next_id: AtomicU64,
    /// The page session, once one is attached.
    session: Arc<Mutex<Option<String>>>,
}

impl Cdp {
    /// Connect to a browser's DevTools WebSocket.
    ///
    /// The URL comes from the browser's own `/json/version`, so it always
    /// points at the process we started.
    pub fn connect(ws_url: &str) -> Result<Self, String> {
        // `Uri::into_client_request` adds the Host header and the key.
        let request: tungstenite::http::Request<()> = ws_url
            .parse::<tungstenite::http::Uri>()
            .map_err(|e| format!("bad endpoint: {e}"))?
            .into_client_request()
            .map_err(|e| format!("bad endpoint: {e}"))?;
        let host = request
            .uri()
            .host()
            .ok_or("the endpoint has no host")?
            .to_string();
        let port = request.uri().port_u16().unwrap_or(9222);

        let stream = std::net::TcpStream::connect((host.as_str(), port))
            .map_err(|e| format!("cannot reach the browser: {e}"))?;
        // The owner does everything, so it must come back for work now and
        // then. A short timeout does that; tungstenite keeps the half-read
        // frame, so nothing is lost.
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .map_err(|e| format!("cannot set the read timeout: {e}"))?;
        let (socket, _) = tungstenite::client::client_with_config(request, stream, None)
            .map_err(|e| format!("cannot reach the browser: {e}"))?;

        let (commands, inbox) = channel();
        let session: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // The socket's owner: reads, writes, and hands replies back.
        {
            let session = session.clone();
            std::thread::Builder::new()
                .name("workspace-cdp".into())
                .spawn(move || {
                    let mut socket = socket;
                    let mut pending: Vec<(u64, Sender<Result<Value, String>>)> = Vec::new();
                    let mut listeners: Vec<Sender<Value>> = Vec::new();
                    loop {
                        // Work first: a caller's command must not wait for a
                        // message that may never come.
                        while let Ok(command) = inbox.try_recv() {
                            match command {
                                Command::Call { id, payload, reply } => {
                                    if socket.send(tungstenite::Message::text(payload)).is_err() {
                                        break;
                                    }
                                    pending.push((id, reply));
                                }
                                Command::Listen(listener) => listeners.push(listener),
                            }
                        }

                        let message = match socket.read() {
                            Ok(m) => m,
                            Err(tungstenite::Error::Io(e)) => {
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) {
                                    continue;
                                }
                                break;
                            }
                            Err(_) => break,
                        };
                        let tungstenite::Message::Text(text) = message else {
                            continue;
                        };
                        let Ok(value) = serde_json::from_str::<Value>(&text) else {
                            continue;
                        };
                        // A reply has an id; an event has a method.
                        if let Some(id) = value.get("id").and_then(Value::as_u64) {
                            let Some(position) =
                                pending.iter().position(|(waiting, _)| *waiting == id)
                            else {
                                continue;
                            };
                            let (_, reply) = pending.remove(position);
                            let result = match value.get("error") {
                                Some(err) => Err(err.to_string()),
                                None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
                            };
                            let _ = reply.send(result);
                            continue;
                        }
                        listeners.retain(|listener| listener.send(value.clone()).is_ok());
                    }
                    // The browser is gone: fail every call still waiting.
                    for (_, reply) in pending {
                        let _ = reply.send(Err("the browser closed the connection".into()));
                    }
                    let _ = &session;
                })
                .map_err(|e| format!("cannot start the socket owner: {e}"))?;
        }

        Ok(Self {
            commands,
            next_id: AtomicU64::new(1),
            session,
        })
    }

    /// Run a command on the browser itself, and wait for the answer.
    pub fn call_browser(&self, method: &str, params: Value) -> Result<Value, String> {
        self.request(method, params, None)
    }

    /// Run a command on the attached page, and wait for the answer.
    pub fn call_page(&self, method: &str, params: Value) -> Result<Value, String> {
        let session = self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "no page is attached".to_string())?;
        self.request(method, params, Some(session))
    }

    /// Open a page and attach to it.
    pub fn attach(&self) -> Result<String, String> {
        let created = self.call_browser("Target.createTarget", json!({ "url": "about:blank" }))?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "the browser did not return a target".to_string())?
            .to_string();

        // Flat mode: the session id rides on every message for this page.
        let attached = self.call_browser(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
        )?;
        let session = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "the browser did not return a session".to_string())?
            .to_string();
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);
        Ok(target_id)
    }

    /// Listen for the page's events.
    pub fn events(&self) -> Receiver<Value> {
        let (tx, rx) = channel();
        // If the owner is gone, the listener simply never hears anything.
        let _ = self.commands.send(Command::Listen(tx));
        rx
    }

    /// Send one command and wait for its answer.
    fn request(
        &self,
        method: &str,
        params: Value,
        session: Option<String>,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = &session {
            message["sessionId"] = json!(session);
        }

        let (tx, rx) = channel();
        self.commands
            .send(Command::Call {
                id,
                payload: message.to_string(),
                reply: tx,
            })
            .map_err(|_| "the browser connection is closed".to_string())?;

        rx.recv_timeout(CALL_TIMEOUT)
            .map_err(|e| format!("{method} did not answer: {e}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page event carries the method name and its parameters.
    #[test]
    fn an_event_is_parsed() {
        let value: Value =
            serde_json::from_str(r#"{"method":"Page.screencastFrame","params":{"data":"AA=="}}"#)
                .unwrap();
        assert_eq!(value["method"], "Page.screencastFrame");
        assert_eq!(value["params"]["data"], "AA==");
    }

    /// A reply is matched by its id, not by arrival order.
    #[test]
    fn replies_carry_their_id() {
        let value: Value = serde_json::from_str(r#"{"id":7,"result":{"ok":true}}"#).unwrap();
        assert_eq!(value.get("id").and_then(Value::as_u64), Some(7));
        assert_eq!(value["result"]["ok"], true);
    }
}
