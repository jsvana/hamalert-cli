use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::error::Error;

const SNR_FROM: &str = "snrFrom";
const SNR_TO: &str = "snrTo";
// HamAlert uses this upper bound in its legacy payload for minimum-only SNR.
const SNR_OPEN_UPPER_BOUND: i32 = 1_000_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct TriggerSettings {
    pub(crate) conditions: Value,
    pub(crate) actions: Vec<String>,
    pub(crate) comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) options: Option<Value>,
}

impl Default for TriggerSettings {
    fn default() -> Self {
        Self {
            conditions: Value::Object(Map::new()),
            actions: Vec::new(),
            comment: String::new(),
            options: None,
        }
    }
}

impl TriggerSettings {
    pub(crate) fn apply_snr(
        &mut self,
        from: Option<i32>,
        to: Option<i32>,
        clear: bool,
    ) -> Result<(), Box<dyn Error>> {
        if !clear && from.is_none() && to.is_none() {
            return Ok(());
        }

        let conditions = self
            .conditions
            .as_object_mut()
            .ok_or("Trigger conditions must be a JSON object")?;
        if clear {
            if from.is_some() || to.is_some() {
                return Err("SNR bounds cannot be set and cleared together".into());
            }
            conditions.remove(SNR_FROM);
            conditions.remove(SNR_TO);
            return Ok(());
        }

        set_snr_bounds(conditions, from, to)
    }

    pub(crate) fn validate(&mut self, original: Option<&Self>) -> Result<(), Box<dyn Error>> {
        let conditions = self
            .conditions
            .as_object_mut()
            .filter(|conditions| !conditions.is_empty())
            .ok_or("Trigger conditions must be a nonempty JSON object")?;

        if let Some(options) = &self.options
            && original.is_none_or(|original| original.options.as_ref() != Some(options))
        {
            // HamAlert's PHP code can represent empty options as an empty array.
            if !options.is_object() && !options.as_array().is_some_and(Vec::is_empty) {
                return Err("Trigger options must be a JSON object".into());
            }
        }

        if original.is_some_and(|original| {
            conditions.get(SNR_FROM) == original.conditions.get(SNR_FROM)
                && conditions.get(SNR_TO) == original.conditions.get(SNR_TO)
        }) {
            return Ok(());
        }

        let from = read_snr_bound(conditions, SNR_FROM)?;
        let to = read_snr_bound(conditions, SNR_TO)?;
        set_snr_bounds(conditions, from, to)
    }
}

fn read_snr_bound(
    conditions: &Map<String, Value>,
    field: &str,
) -> Result<Option<i32>, Box<dyn Error>> {
    let Some(value) = conditions.get(field) else {
        return Ok(None);
    };
    value
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| format!("{field} must be a signed 32-bit integer in dB").into())
}

fn set_snr_bounds(
    conditions: &mut Map<String, Value>,
    from: Option<i32>,
    to: Option<i32>,
) -> Result<(), Box<dyn Error>> {
    let from = match (from, to) {
        (Some(from), _) => from,
        (None, Some(_)) => return Err("snrTo requires snrFrom".into()),
        (None, None) => return Ok(()),
    };
    let to = to.unwrap_or(SNR_OPEN_UPPER_BOUND);
    if from > to {
        return Err("snrFrom must be less than or equal to snrTo".into());
    }

    conditions.insert(SNR_FROM.to_owned(), Value::from(from));
    conditions.insert(SNR_TO.to_owned(), Value::from(to));
    Ok(())
}

#[derive(Serialize)]
struct TriggerRequest<'a> {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    conditions: &'a Value,
    actions: &'a [String],
    comment: &'a str,
    options: &'a Value,
}

pub(crate) async fn save_trigger(
    client: &Client,
    url: &str,
    id: Option<&str>,
    settings: &TriggerSettings,
) -> Result<(), Box<dyn Error>> {
    let empty_options = Value::Object(Map::new());
    let request = TriggerRequest {
        id,
        conditions: &settings.conditions,
        actions: &settings.actions,
        comment: &settings.comment,
        options: settings.options.as_ref().unwrap_or(&empty_options),
    };
    let response = client.post(url).json(&request).send().await?;
    let status = response.status();
    let body = response.text().await?;
    let body = body.trim();

    let reason = if !status.is_success() {
        status.to_string()
    } else if matches!(serde_json::from_str::<bool>(body), Ok(true)) {
        return Ok(());
    } else {
        "HamAlert did not confirm the save".to_owned()
    };

    let operation = if id.is_some() { "update" } else { "create" };
    let message = format!(
        "Failed to {operation} trigger {:?}: {reason}",
        settings.comment
    );
    if body.is_empty() {
        Err(message.into())
    } else {
        Err(format!("{message}: {body}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    #[test]
    fn snr_flags_preserve_replace_and_clear_the_range() {
        let mut settings = TriggerSettings {
            conditions: json!({"band": "20m", "snrFrom": -25, "snrTo": 15}),
            ..TriggerSettings::default()
        };

        settings.apply_snr(None, None, false).unwrap();
        assert_eq!(settings.conditions["snrTo"], 15);
        settings.apply_snr(Some(-20), None, false).unwrap();
        assert_eq!(settings.conditions["snrTo"], 1_000_000);
        settings.apply_snr(Some(-10), Some(-10), false).unwrap();
        assert_eq!(settings.conditions["snrFrom"], -10);
        assert_eq!(settings.conditions["snrTo"], -10);
        assert!(settings.apply_snr(None, Some(12), false).is_err());
        assert!(settings.apply_snr(Some(12), Some(10), false).is_err());
        assert!(settings.apply_snr(Some(1_000_001), None, false).is_err());
        assert_eq!(settings.conditions["snrFrom"], -10);
        assert_eq!(settings.conditions["snrTo"], -10);
        settings.apply_snr(None, None, true).unwrap();
        assert_eq!(settings.conditions, json!({"band": "20m"}));
    }

    #[test]
    fn raw_settings_validate_and_fill_a_missing_snr_upper_bound() {
        let mut settings: TriggerSettings = serde_json::from_value(json!({
            "conditions": {"band": ["20m", "40m"], "snrFrom": -20, "future": [42]},
            "actions": ["app"],
            "comment": "Band alert",
            "options": {"future": {"enabled": true}}
        }))
        .unwrap();
        settings.validate(None).unwrap();
        assert_eq!(
            settings.conditions,
            json!({"band": ["20m", "40m"], "snrFrom": -20, "snrTo": 1_000_000, "future": [42]})
        );
        assert_eq!(settings.options, Some(json!({"future": {"enabled": true}})));
    }

    #[test]
    fn raw_settings_reject_invalid_shapes_and_snr_ranges() {
        for conditions in [
            json!({}),
            json!([]),
            json!({"band": "20m", "snrTo": -10}),
            json!({"band": "20m", "snrFrom": -10, "snrTo": -20}),
            json!({"band": "20m", "snrFrom": "-20"}),
            json!({"band": "20m", "snrFrom": -20.5}),
            json!({"band": "20m", "snrFrom": i64::from(i32::MIN) - 1}),
        ] {
            let mut settings = TriggerSettings {
                conditions,
                ..TriggerSettings::default()
            };
            assert!(settings.validate(None).is_err(), "{settings:?}");
        }

        let mut settings = TriggerSettings {
            conditions: json!({"band": "20m", "snrFrom": 0, "snrTo": 0}),
            options: Some(json!([1])),
            ..TriggerSettings::default()
        };
        assert!(settings.validate(None).is_err());
        settings.options = Some(json!([]));
        settings.validate(None).unwrap();
        assert_eq!(settings.conditions["snrFrom"], 0);
        assert_eq!(settings.conditions["snrTo"], 0);
        assert_eq!(settings.options, Some(json!([])));
    }

    #[test]
    fn editing_other_settings_preserves_legacy_snr_and_options() {
        let original = TriggerSettings {
            conditions: json!({"band": "20m", "snrFrom": "-20"}),
            options: Some(json!(["legacy"])),
            ..TriggerSettings::default()
        };
        let mut edited = original.clone();
        edited.comment = "Band alert".to_owned();
        edited.validate(Some(&original)).unwrap();
        assert_eq!(edited.conditions, original.conditions);
        assert_eq!(edited.options, original.options);

        edited.conditions["snrFrom"] = json!(-20);
        edited.validate(Some(&original)).unwrap();
        assert_eq!(edited.conditions["snrTo"], 1_000_000);
    }

    #[tokio::test]
    async fn save_trigger_uses_hamalert_wire_contract() {
        let client = Client::builder().no_proxy().build().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/ajax/trigger_update",
            listener.local_addr().unwrap()
        );
        let mut settings = TriggerSettings {
            conditions: json!({"band": ["20m", "40m"], "snrFrom": -20, "snrTo": 10}),
            actions: vec!["app".to_owned()],
            comment: "Band alert".to_owned(),
            options: None,
        };
        let id = "1234567890abcdef12345678";

        for (trigger_id, status, response, error) in [
            (None, "200 OK", "true", None),
            (Some(id), "200 OK", "true", None),
            (
                None,
                "400 Bad Request",
                "Invalid trigger",
                Some("400 Bad Request: Invalid trigger"),
            ),
            (
                Some(id),
                "200 OK",
                "false",
                Some("HamAlert did not confirm the save: false"),
            ),
            (
                Some(id),
                "200 OK",
                "{}",
                Some("HamAlert did not confirm the save: {}"),
            ),
        ] {
            let mut expected = json!({
                "conditions": settings.conditions,
                "actions": settings.actions,
                "comment": settings.comment,
                "options": {}
            });
            if let Some(trigger_id) = trigger_id {
                settings.options = Some(json!({"future": true}));
                expected["_id"] = json!(trigger_id);
                expected["options"] = json!({"future": true});
            } else {
                settings.options = None;
            }

            let server = async {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                let mut request_line = String::new();
                socket.read_line(&mut request_line).await.unwrap();
                assert_eq!(request_line, "POST /ajax/trigger_update HTTP/1.1\r\n");

                let mut content_length = None;
                let mut headers = (&mut socket).lines();
                while let Some(header) = headers.next_line().await.unwrap() {
                    if header.is_empty() {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap();
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = Some(value.trim().parse().unwrap());
                    }
                }
                let mut body = vec![0; content_length.unwrap()];
                socket.read_exact(&mut body).await.unwrap();
                assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), expected);

                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            };
            let (result, ()) = timeout(Duration::from_secs(5), async {
                tokio::join!(
                    save_trigger(&client, &endpoint, trigger_id, &settings),
                    server
                )
            })
            .await
            .expect("request to the local test server timed out");

            if let Some(error) = error {
                let message = result.unwrap_err().to_string();
                assert!(message.contains(error), "{message}");
            } else {
                result.unwrap();
            }
        }
    }
}
