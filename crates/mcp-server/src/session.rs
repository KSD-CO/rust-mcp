use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

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
    pub client_info: Option<mcp_core::types::ClientInfo>,
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

/// Shared session store
#[derive(Clone, Default)]
pub struct SessionStore {
    sessions: Arc<DashMap<SessionId, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, session: Session) {
        self.sessions.insert(session.id.clone(), session);
    }

    pub fn get(&self, id: &SessionId) -> Option<Session> {
        self.sessions.get(id).map(|s| s.clone())
    }

    pub fn update<F>(&self, id: &SessionId, f: F)
    where
        F: FnOnce(&mut Session),
    {
        if let Some(mut session) = self.sessions.get_mut(id) {
            f(&mut session);
        }
    }

    pub fn remove(&self, id: &SessionId) {
        self.sessions.remove(id);
    }
}
