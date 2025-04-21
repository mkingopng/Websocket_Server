use crate::auth::{AuthService, SessionManager};
use crate::messages::Session;
use async_trait::async_trait;

pub struct DefaultAuth {
    sm: SessionManager,
}

impl DefaultAuth {
    pub fn new(sm: SessionManager) -> Self {
        Self { sm }
    }
}

#[async_trait]
impl AuthService for DefaultAuth {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let session = self
            .sm
            .create_session(meet_id, location_name, priority)
            .await;
        session.token
    }

    async fn get_session(&self, token: &str) -> Option<Session> {
        self.sm.get_session(token).await
    }

    async fn validate_session(&self, token: &str) -> bool {
        self.sm.validate_session(token).await
    }
}
