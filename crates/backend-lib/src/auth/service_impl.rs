use super::{AuthService, PasswordRequirements, Session, SessionManager};
use super::password::{hash_password, verify_password, validate_password_strength};
use async_trait::async_trait;

pub struct DefaultAuth {
    sm: SessionManager,
}

impl DefaultAuth {
    pub fn new(sm: SessionManager) -> Self { Self { sm } }
}

#[async_trait]
impl AuthService for DefaultAuth {
    async fn hash_password(&self, p: &str) -> anyhow::Result<String> { 
        hash_password(p) 
    }
    
    fn verify_password(&self, h: &str, p: &str) -> bool { 
        verify_password(h, p) 
    }
    
    fn password_ok(&self, pwd: &str, req: &PasswordRequirements) -> bool {
        validate_password_strength(pwd, req)
    }
    
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        self.sm.new_session(meet_id, location_name, priority)
    }
    
    async fn get_session(&self, token: &str) -> Option<Session> {
        self.sm.get_session(token)
    }
} 