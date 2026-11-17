//! VK launch proof, following VKCOM's launch-params algorithm and the adjacent Blitz app.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Config {
    pub app_id: u64,
    pub secret: String,
}
fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
fn payload(fields: &BTreeMap<String, String>) -> String {
    fields
        .iter()
        .filter(|(k, _)| k.starts_with("vk_"))
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}
pub fn verify(query: &str, config: &Config, now: u64) -> Result<i64, &'static str> {
    if query.len() > 16_384 || config.secret.is_empty() || config.app_id == 0 {
        return Err("invalid VK configuration or proof");
    }
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_str(query.trim_start_matches('?')).map_err(|_| "invalid query")?;
    let mut fields = BTreeMap::new();
    for (k, v) in pairs {
        if fields.insert(k, v).is_some() {
            return Err("duplicate field");
        }
    }
    let given = URL_SAFE_NO_PAD
        .decode(fields.get("sign").ok_or("missing sign")?)
        .map_err(|_| "invalid sign")?;
    let mut mac = Hmac::<Sha256>::new_from_slice(config.secret.as_bytes()).unwrap();
    mac.update(payload(&fields).as_bytes());
    mac.verify_slice(&given).map_err(|_| "invalid sign")?;
    let number = |name| {
        fields
            .get(name)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("invalid field")
    };
    let id = number("vk_user_id")?;
    let app = number("vk_app_id")?;
    let ts = number("vk_ts")?;
    if app != config.app_id
        || id == 0
        || id > ((1u64 << 52) - 1)
        || ts > now.saturating_add(30)
        || now.saturating_sub(ts) > 86_400
    {
        return Err("invalid app, user or expired proof");
    }
    Ok(id as i64)
}
#[cfg(test)]
pub(crate) fn signed(id: i64, app: u64, ts: u64) -> String {
    let fields = BTreeMap::from([
        ("vk_app_id".into(), app.to_string()),
        ("vk_user_id".into(), id.to_string()),
        ("vk_ts".into(), ts.to_string()),
        ("vk_ref".into(), "other/с пробелом&x=1".into()),
    ]);
    let mut mac = Hmac::<Sha256>::new_from_slice(b"vk-test-secret").unwrap();
    mac.update(payload(&fields).as_bytes());
    let sign = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!(
        "{}&sign={sign}",
        serde_urlencoded::to_string(fields).unwrap()
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_and_signature() {
        assert_eq!(encode("a b/ц!~*'()"), "a%20b%2F%D1%86!~*'()");
        let c = Config {
            app_id: 123,
            secret: "vk-test-secret".into(),
        };
        let q = signed(777, 123, 1000);
        assert_eq!(verify(&q, &c, 1500), Ok(777));
        assert!(verify(&q.replace("vk_user_id=777", "vk_user_id=778"), &c, 1500).is_err());
        assert!(verify(&(q.clone() + "&vk_user_id=777"), &c, 1500).is_err());
        assert!(verify(&(q.clone() + "&sign=x"), &c, 1500).is_err());
        assert!(
            verify(
                &q,
                &Config {
                    app_id: 123,
                    secret: "wrong".into()
                },
                1500
            )
            .is_err()
        );
        assert!(verify("vk_user_id=777&vk_app_id=123&vk_ts=1000", &c, 1500).is_err());
        assert_eq!(
            verify(&(q + "&deal=t1_1_ffffffffffffffff"), &c, 1500),
            Ok(777)
        );
    }
    #[test]
    fn app_age_ids_and_disabled() {
        let c = Config {
            app_id: 123,
            secret: "vk-test-secret".into(),
        };
        for (id, app, ts) in [(1, 124, 1000), (0, 123, 1000), (-1, 123, 1000), (1, 123, 1)] {
            assert!(verify(&signed(id, app, ts), &c, 86_500).is_err());
        }
        assert!(verify(&signed(1, 123, 1531), &c, 1500).is_err());
        assert!(
            verify(
                &signed(1, 123, 1000),
                &Config {
                    app_id: 123,
                    secret: String::new()
                },
                1500
            )
            .is_err()
        );
    }
}
