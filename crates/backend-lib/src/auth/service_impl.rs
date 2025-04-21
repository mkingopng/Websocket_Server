use super::*;
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
    
    async fn new_session(&self, m: String, l: String, p: u8) -> String {
        self.sm.new_session(m, l, p).await
    }
    
    async fn get_session(&self, t: &str) -> Option<Session> {
        self.sm.get(t).await
    }
} 