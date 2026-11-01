// Private-message transport envelopes from Horizon's server decoration.
// Never infer an authenticated sender from a rendered chat line or frame data.
use serde_json::Value;

pub const PREFIX: &str = "[DHP1] ";
pub const MAX_BODY_BYTES: usize = 180;
const MAX_NODES: usize = 256;
const MAX_COMPONENT_BYTES: usize = 8192;
const MAX_DEPTH: usize = 16;

/// Return the sender's UUID, canonical online account name and complete literal
/// protocol body. This validates transport provenance only; the party model
/// must still validate the frame, invitation, friendship, nonce and session.
pub fn incoming(component: &Value, online: &[(u128, String)]) -> Option<(u128, String, String)> {
    decode(component, online, "commands.message.display.incoming")
}

/// Decode the server's echo to the sender. The returned identity is the
/// recipient. Only hide it after matching an exact recently sent local frame.
pub fn outgoing(component: &Value, online: &[(u128, String)]) -> Option<(u128, String, String)> {
    decode(component, online, "commands.message.display.outgoing")
}

fn decode(
    component: &Value,
    online: &[(u128, String)],
    key: &str,
) -> Option<(u128, String, String)> {
    if !bounded(component, 0, &mut 0, &mut 0)
        || component.get("translate")?.as_str()? != key
        || nonempty_siblings(component)
    {
        return None;
    }
    let args = component.get("with")?.as_array()?;
    if args.len() != 2 {
        return None;
    }
    let body = literal(&args[1])?;
    if !body.starts_with(PREFIX)
        || body.len() > MAX_BODY_BYTES
        || body.len() == PREFIX.len()
        || body.chars().any(char::is_control)
    {
        return None;
    }
    let mut identity = None;
    sender_identity(&args[0], 0, &mut identity)?;
    let (uuid, name) = identity?;
    let profile = online.iter().find(|(id, profile)| {
        *id == uuid && account_name(profile) && profile.eq_ignore_ascii_case(&name)
    })?;
    Some((uuid, profile.1.clone(), body.to_owned()))
}

fn account_name(value: &str) -> bool {
    (1..=16).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn literal(value: &Value) -> Option<&str> {
    if let Some(s) = value.as_str() {
        return Some(s);
    }
    if value.get("translate").is_some()
        || value.get("selector").is_some()
        || value.get("score").is_some()
        || value.get("nbt").is_some()
        || nonempty_siblings(value)
    {
        return None;
    }
    value.get("text")?.as_str()
}

fn nonempty_siblings(value: &Value) -> bool {
    value
        .get("extra")
        .is_some_and(|v| !v.as_array().is_some_and(Vec::is_empty))
}

fn bounded(value: &Value, depth: usize, nodes: &mut usize, bytes: &mut usize) -> bool {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return false;
    }
    match value {
        Value::String(s) => *bytes += s.len(),
        Value::Array(a) => {
            for item in a {
                if !bounded(item, depth + 1, nodes, bytes) {
                    return false;
                }
            }
        }
        Value::Object(o) => {
            for (key, item) in o {
                *bytes += key.len();
                if !bounded(item, depth + 1, nodes, bytes) {
                    return false;
                }
            }
        }
        _ => {}
    }
    *bytes <= MAX_COMPONENT_BYTES
}

fn sender_identity(
    sender: &Value,
    depth: usize,
    result: &mut Option<(u128, String)>,
) -> Option<()> {
    if depth > MAX_DEPTH {
        return None;
    }
    if let Some(event) = sender.get("hover_event") {
        if event.get("action")?.as_str()? != "show_entity"
            || event.get("id")?.as_str()? != "minecraft:player"
        {
            return None;
        }
        let uuid = uuid(event.get("uuid")?)?;
        let name = literal(event.get("name")?)?;
        if !account_name(name) {
            return None;
        }
        if let Some((old_id, old_name)) = result {
            if *old_id != uuid || !old_name.eq_ignore_ascii_case(name) {
                return None;
            }
        } else {
            *result = Some((uuid, name.to_owned()));
        }
    }
    // Team formatting can place the name's component inside literal siblings.
    // Do not inspect arbitrary hover text, click commands or frame payloads.
    for key in ["extra", "with"] {
        if let Some(parts) = sender.get(key) {
            for part in parts.as_array()? {
                sender_identity(part, depth + 1, result)?;
            }
        }
    }
    Some(())
}

fn uuid(value: &Value) -> Option<u128> {
    if let Some(words) = value.as_array() {
        if words.len() != 4 {
            return None;
        }
        let mut result = 0u128;
        for word in words {
            let word = i32::try_from(word.as_i64()?).ok()?;
            result = (result << 32) | u128::from(word as u32);
        }
        return Some(result);
    }
    let text = value.as_str()?;
    if text.len() != 36
        || [8, 13, 18, 23].iter().any(|&p| text.as_bytes()[p] != b'-')
        || text
            .bytes()
            .enumerate()
            .any(|(p, c)| ![8, 13, 18, 23].contains(&p) && !c.is_ascii_hexdigit())
    {
        return None;
    }
    u128::from_str_radix(&text.replace('-', ""), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const UUID: u128 = 0x123456789abcdef0fedcba9876543210;

    fn envelope() -> Value {
        json!({
            "translate": "commands.message.display.incoming",
            "with": [{
                "text": "~ShownNickname",
                "insertion": "RealAccount",
                "hover_event": {
                    "action": "show_entity", "id": "minecraft:player",
                    "uuid": "12345678-9abc-def0-fedc-ba9876543210", "name": "RealAccount"
                }
            }, {"text": "[DHP1] I 12345678"}]
        })
    }

    fn online() -> Vec<(u128, String)> {
        vec![(UUID, "RealAccount".to_owned())]
    }

    #[test]
    fn accepts_real_identity_despite_nickname_and_localized_display() {
        assert_eq!(
            incoming(&envelope(), &online()),
            Some((UUID, "RealAccount".into(), "[DHP1] I 12345678".into()))
        );
    }

    #[test]
    fn outgoing_echo_returns_authenticated_recipient_not_incoming_sender() {
        let mut e = envelope();
        e["translate"] = "commands.message.display.outgoing".into();
        assert!(incoming(&e, &online()).is_none());
        assert_eq!(
            outgoing(&e, &online()),
            Some((UUID, "RealAccount".into(), "[DHP1] I 12345678".into()))
        );
        assert!(outgoing(&envelope(), &online()).is_none());
    }

    #[test]
    fn accepts_nbt_signed_integer_uuid_words() {
        let mut e = envelope();
        e["with"][0]["hover_event"]["uuid"] = json!([
            0x12345678i32,
            0x9abcdef0u32 as i32,
            0xfedcba98u32 as i32,
            0x76543210i32
        ]);
        assert!(incoming(&e, &online()).is_some());
    }

    #[test]
    fn rejects_unknown_offline_and_mismatched_senders() {
        assert!(incoming(&envelope(), &[]).is_none());
        assert!(incoming(&envelope(), &[(UUID + 1, "RealAccount".into())]).is_none());
        let mut e = envelope();
        e["with"][0]["hover_event"]["name"] = "OtherPlayer".into();
        assert!(incoming(&e, &online()).is_none());
    }

    #[test]
    fn public_chat_emote_outgoing_and_plain_text_cannot_impersonate_whisper() {
        for key in [
            "chat.type.text",
            "chat.type.emote",
            "commands.message.display.outgoing",
        ] {
            let mut e = envelope();
            e["translate"] = key.into();
            assert!(incoming(&e, &online()).is_none());
        }
        assert!(
            incoming(
                &json!({"text":"RealAccount whispers to you: [DHP1] I 12345678"}),
                &online()
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_forged_body_formatting_and_hidden_siblings() {
        let mut e = envelope();
        e["with"][1] = json!({"text":"[DHP1] I 12345678", "extra":[{"text":"malicious suffix"}]});
        assert!(incoming(&e, &online()).is_none());
        let mut e = envelope();
        e["extra"] = json!([{"text":"malicious root suffix"}]);
        assert!(incoming(&e, &online()).is_none());
        let mut e = envelope();
        e["with"][1] = json!({"translate":"fake.key", "text":"[DHP1] I 12345678"});
        assert!(incoming(&e, &online()).is_none());
    }

    #[test]
    fn supports_literal_body_and_nested_team_name_identity() {
        let mut e = envelope();
        e["with"][1] = "[DHP1] I 12345678".into();
        let sender = e["with"][0].take();
        e["with"][0] = json!({"text":"[Team] ","extra":[sender]});
        assert!(incoming(&e, &online()).is_some());
    }

    #[test]
    fn rejects_oversized_control_character_and_ambiguous_frames() {
        let mut e = envelope();
        e["with"][1] = format!("[DHP1] {}", "x".repeat(MAX_BODY_BYTES)).into();
        assert!(incoming(&e, &online()).is_none());
        e["with"][1] = "[DHP1] I\nnext".into();
        assert!(incoming(&e, &online()).is_none());
        e["with"][1] = PREFIX.into();
        assert!(incoming(&e, &online()).is_none());
        e = envelope();
        e["with"]
            .as_array_mut()
            .unwrap()
            .push(json!("third argument"));
        assert!(incoming(&e, &online()).is_none());
        e = envelope();
        e["with"][0]["extra"] = json!([{"hover_event":{"action":"show_entity","id":"minecraft:player","uuid":"00000000-0000-0000-0000-000000000001","name":"Other"}}]);
        assert!(incoming(&e, &online()).is_none());
    }

    #[test]
    fn bounds_component_depth_nodes_and_text_before_parsing_identity() {
        let mut e = envelope();
        e["unrelated"] = "x".repeat(MAX_COMPONENT_BYTES + 1).into();
        assert!(incoming(&e, &online()).is_none());
        let mut nested = json!(null);
        for _ in 0..MAX_DEPTH + 2 {
            nested = json!([nested]);
        }
        e = envelope();
        e["unrelated"] = nested;
        assert!(incoming(&e, &online()).is_none());
        e = envelope();
        e["unrelated"] = json!(vec![0; MAX_NODES + 1]);
        assert!(incoming(&e, &online()).is_none());
    }
}
