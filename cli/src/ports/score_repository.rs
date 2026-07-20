pub trait ScoreRepository {
    /// Reads `.heist/<slug>/score.md`. `Ok(None)` means the file doesn't
    /// exist (score.md is optional until the Forger writes it); `Err` is a
    /// true IO failure (permissions, corrupt filesystem, etc).
    fn load_score(&self, slug: &str) -> Result<Option<String>, std::io::Error>;
}
