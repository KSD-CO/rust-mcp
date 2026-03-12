use uuid::Uuid;

use crate::types::ClientInfo;

/// Unique session identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Per-connection session data
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub client_info: Option<ClientInfo>,
    pub protocol_version: Option<String>,
    pub initialized: bool,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: SessionId::new(),
            client_info: None,
            protocol_version: None,
            initialized: false,
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
