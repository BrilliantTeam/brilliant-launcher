use base64::Engine;

const PREFIX: &str = "dpapi:";

pub fn needs_seal(text: &str) -> bool {
    cfg!(windows) && !text.is_empty() && !text.starts_with(PREFIX)
}

pub fn seal(text: &str) -> String {
    if !needs_seal(text) {
        return text.to_string();
    }
    #[cfg(windows)]
    if let Some(blob) = dpapi::run(text.as_bytes(), true) {
        return format!("{PREFIX}{}", base64::engine::general_purpose::STANDARD.encode(blob));
    }
    text.to_string()
}

pub fn open(text: &str) -> Option<String> {
    let Some(encoded) = text.strip_prefix(PREFIX) else {
        return Some(text.to_string());
    };
    let blob = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    #[cfg(windows)]
    {
        dpapi::run(&blob, false).and_then(|plain| String::from_utf8(plain).ok())
    }
    #[cfg(not(windows))]
    {
        let _ = blob;
        None
    }
}

#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    pub fn run(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: u32::try_from(data.len()).ok()?, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = unsafe {
            if protect {
                CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            }
        };
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe { LocalFree(output.pbData as _) };
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_tokens_round_trip_and_plain_text_still_reads() {
        let token = "M.C540_BAY.0.U.-refresh-token";
        let sealed = seal(token);
        if cfg!(windows) {
            assert!(sealed.starts_with(PREFIX) && !sealed.contains(token));
        }
        assert_eq!(open(&sealed).as_deref(), Some(token));
        assert_eq!(seal(&sealed), sealed);
        assert_eq!(open(token).as_deref(), Some(token));
        assert_eq!(seal(""), "");
        assert_eq!(open("dpapi:not-base64!"), None);
        assert_eq!(open("dpapi:AAAA"), None);
    }
}
