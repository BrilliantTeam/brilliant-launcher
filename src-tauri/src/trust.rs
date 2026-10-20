use base64::Engine;

pub const PUBLIC_KEY: &str = "lunfr7RSo4DILBeY6dlwBW8KpSFkbSMweYK0DLB/uMU=";

pub fn verify(file: &str, bytes: &[u8], signature: &str) -> bool {
    let engine = base64::engine::general_purpose::STANDARD;
    let (Ok(key), Ok(sig)) = (engine.decode(PUBLIC_KEY), engine.decode(signature.trim())) else {
        return false;
    };
    let message = format!("brilliant-update-v1\n{file}\n{}", crate::modinject::sha256_hex(bytes));
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(message.as_bytes(), &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "BrilliantLauncher_9.9.9_x64-setup.exe";
    const SIG: &str = "7w50zp16kSKorY4aE6TR6E0MSnEG1MWHblvMoE1SKd4SdPuvVdYHrENDjCatgMxruSwFyAP9PgIc+m82xxznCg==";

    #[test]
    fn accepts_only_signed_files() {
        assert!(verify(NAME, b"hello", SIG));
        assert!(!verify(NAME, b"hellp", SIG), "內容被改過要拒絕");
        assert!(!verify("BrilliantLauncher_9.9.9_universal.dmg", b"hello", SIG), "改名冒充要拒絕");
        assert!(!verify(NAME, b"hello", ""), "沒有簽章要拒絕");
        assert!(!verify(NAME, b"hello", "not base64!"));
    }
}
