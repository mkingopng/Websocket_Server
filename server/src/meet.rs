// server/src/meet.rs
use std::sync::Arc;
use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct Meet {
    pub id: String,
    pub name: String,
    pub owner: String,
}

#[derive(Debug, Clone)]
pub struct MeetManager {
    meets: Arc<DashMap<String, Meet>>,
}

impl MeetManager {
    pub fn new() -> Self {
        Self {
            meets: Arc::new(DashMap::new()),
        }
    }

    pub fn create_meet(&self, id: String, name: String, owner: String) -> Meet {
        let meet = Meet { id: id.clone(), name, owner };
        self.meets.insert(id, meet.clone());
        meet
    }

    pub fn get_meet(&self, id: &str) -> Option<Meet> {
        self.meets.get(id).map(|meet| meet.clone())
    }

    pub fn delete_meet(&self, id: &str) {
        self.meets.remove(id);
    }
}

impl Default for MeetManager {
    fn default() -> Self {
        Self::new()
    }
} 