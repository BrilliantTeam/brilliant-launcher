pub mod msa;

use crate::config::Account;
use crate::err::R;
use crate::bail;
use md5::Digest;

pub fn dash_uuid(raw: &str) -> String {
    let hex: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return raw.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub fn offline_uuid(name: &str) -> String {
    let mut hash = md5::Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    hash[6] = (hash[6] & 0x0f) | 0x30;
    hash[8] = (hash[8] & 0x3f) | 0x80;
    dash_uuid(&hex::encode(hash))
}

pub fn make_offline(name: &str) -> R<Account> {
    let name = name.trim();
    if !(3..=16).contains(&name.chars().count())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("離線名稱只能用 3～16 個英數字或底線");
    }
    Ok(Account {
        id: format!("offline:{name}"),
        kind: "offline".into(),
        name: name.to_string(),
        uuid: offline_uuid(name),
        access_token: "0".into(),
        refresh_token: String::new(),
        xuid: String::new(),
        expires_at: 0,
        session_error: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(offline_uuid("jeb_"), "a762f560-4fce-3236-812a-b80efff0b62b");
        assert_eq!(offline_uuid("Rice_07"), "d1cee436-ecaf-3011-a8f2-dc9ce493c27b");
    }

    #[test]
    fn dashing_is_idempotent() {
        let dashed = dash_uuid("069a79f444e94726a5befca90e38aaf5");
        assert_eq!(dashed, "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        assert_eq!(dash_uuid(&dashed), dashed);
    }

    #[test]
    fn offline_name_is_validated() {
        assert!(make_offline("ab").is_err());
        assert!(make_offline("has space").is_err());
        assert!(make_offline("Rice_07").is_ok());
    }
}
