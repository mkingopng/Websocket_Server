use async_trait::async_trait;
use super::{Session, SessionManager, PasswordRequirements};

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn hash_password(&self, plain: &str) -> anyhow::Result<String>;
    fn verify_password(&self, hash: &str, plain: &str) -> bool;
    fn password_ok(&self, pwd: &str, req: &PasswordRequirements) -> bool;
    async fn new_session(&self, meet: String, loc: String, prio: u8) -> String;
    async fn get_session(&self, token: &str) -> Option<Session>;
} 