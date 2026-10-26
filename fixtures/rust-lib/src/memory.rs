/// A stored memory.
#[derive(Debug, Clone)]
pub struct Memory {
    pub id: u64,
    pub text: String,
}

impl Memory {
    pub fn new(id: u64, text: &str) -> Self {
        Self { id, text: text.to_owned() }
    }
}
