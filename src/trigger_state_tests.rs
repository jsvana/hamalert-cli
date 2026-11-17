use super::{Trigger, set_trigger_disabled};
use reqwest::Client;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::timeout;

#[tokio::test]
async fn pause_and_resume_use_hamalert_wire_contract() {
    const ID: &str = "1234567890abcdef12345678";

    let client = Client::builder().no_proxy().build().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/ajax/trigger_disable",
        listener.local_addr().unwrap()
    );
    let trigger: Trigger = serde_json::from_value(serde_json::json!({
        "_id": ID,
        "conditions": {"callsign": "K3LR"},
        "actions": ["app"],
        "comment": "Contest alert"
    }))
    .unwrap();
    let expected_id = format!("id={ID}");

    for (disabled, status, response, expected_ok) in [
        (true, "200 OK", "true", true),
        (false, "200 OK", "true", true),
        (true, "200 OK", "false", false),
        (false, "500 Internal Server Error", "true", false),
    ] {
        let expected_state = if disabled { "disabled=1" } else { "disabled=0" };
        let server = async {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = BufReader::new(socket);
            let mut request = String::new();
            socket.read_line(&mut request).await.unwrap();
            assert_eq!(request, "POST /ajax/trigger_disable HTTP/1.1\r\n");

            let mut headers = (&mut socket).lines();
            let mut content_length = None;
            let mut content_type = None;
            while let Some(line) = headers.next_line().await.unwrap() {
                if line.is_empty() {
                    break;
                }
                let (name, value) = line.split_once(':').unwrap();
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = Some(value.trim().parse::<usize>().unwrap());
                } else if name.eq_ignore_ascii_case("content-type") {
                    content_type = Some(value.trim().to_owned());
                }
            }
            assert_eq!(
                content_type.as_deref(),
                Some("application/x-www-form-urlencoded")
            );
            let body_length = expected_state.len() + 1 + expected_id.len();
            assert_eq!(content_length, Some(body_length));

            let mut body = vec![0; body_length];
            socket.read_exact(&mut body).await.unwrap();
            let mut fields: Vec<_> = std::str::from_utf8(&body).unwrap().split('&').collect();
            fields.sort_unstable();
            assert_eq!(fields, [expected_state, expected_id.as_str()]);

            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        };
        let (result, ()) = timeout(Duration::from_secs(5), async {
            tokio::join!(
                set_trigger_disabled(&client, &endpoint, &trigger, disabled),
                server
            )
        })
        .await
        .expect("request to the local test server timed out");
        assert_eq!(
            result.is_ok(),
            expected_ok,
            "{status} {response}: {result:?}"
        );
    }
}
