use crate::Memory;

/// Persistence boundary.
pub trait Storage {
    fn put(&mut self, memory: Memory);
    fn get(&self, id: u64) -> Option<&Memory>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}
