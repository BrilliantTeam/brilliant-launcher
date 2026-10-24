pub struct Error(pub anyhow::Error);

pub type R<T> = std::result::Result<T, Error>;

pub fn message(error: &anyhow::Error) -> String {
    let mut text = format!("{error:#}");
    let denied = error.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied));
    if denied {
        text.push_str("
檔案可能正被其他程式使用（遊戲、檔案總管、防毒軟體或雲端同步），請關閉後再試一次；仍然失敗的話，請確認這個資料夾沒有被設成唯讀。");
    }
    text
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&message(&self.0))
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::err::Error(::anyhow::anyhow!($($arg)*)))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn permission_errors_explain_the_likely_cause() {
        let denied = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).context("移到回收資料夾失敗");
        assert!(super::message(&denied).starts_with("移到回收資料夾失敗") && super::message(&denied).contains("其他程式使用"));
        assert!(!super::message(&anyhow::anyhow!("找不到")).contains("其他程式"));
    }
}
