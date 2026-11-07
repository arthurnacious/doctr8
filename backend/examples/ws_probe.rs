//! Tiny WebSocket client for manual testing and the e2e suite.
//!
//!   cargo run --example ws_probe -- <ws-url> <access-token> [account-id ...]
//!
//! Authenticates, subscribes to the given accounts, then prints every server
//! message as one JSON line until the connection closes or it is killed.

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: ws_probe <ws-url> <token> [account-id ...]");
    let token = args.next().expect("missing token");
    let accounts: Vec<String> = args.collect();

    let (mut ws, _) = connect_async(url.as_str()).await.expect("connect");
    let auth = serde_json::json!({"type": "auth", "token": token});
    ws.send(Message::Text(auth.to_string().into())).await.expect("send auth");
    for a in &accounts {
        let sub = serde_json::json!({"type": "subscribe", "account_id": a});
        ws.send(Message::Text(sub.to_string().into())).await.expect("send subscribe");
    }

    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Text(t)) => println!("{t}"),
            Ok(Message::Close(frame)) => {
                println!("{}", serde_json::json!({"closed": frame.map(|f| u16::from(f.code)).unwrap_or(0)}));
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("ws error: {e}");
                break;
            }
        }
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}
