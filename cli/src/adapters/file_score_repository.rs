use crate::ports::score_repository::ScoreRepository;
use std::path::{Path, PathBuf};

pub struct FileScoreRepository;

impl ScoreRepository for FileScoreRepository {
    fn load_score(&self, slug: &str) -> Result<Option<String>, std::io::Error> {
        let path = score_file_path(slug);
        if !path.exists() {
            return Ok(None);
        }
        std::fs::read_to_string(path).map(Some)
    }
}

fn score_file_path(slug: &str) -> PathBuf {
    Path::new(".heist").join(slug).join("score.md")
}
