use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthState {
    Closed,
    WaitParameters,
    WaitPhoneNumber,
    WaitCode(CodeInfo),
    WaitPassword(PasswordHint),
    WaitRegistration,
    WaitQrCode(QrCodeInfo),
    Ready,
    LoggingOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeInfo {
    pub phone_number: String,
    pub timeout: i32,
    pub code_type: String,
    pub next_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordHint {
    pub hint: Option<String>,
    pub has_recovery_email: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrCodeInfo {
    pub link: String,
}
