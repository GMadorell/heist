pub const SUCCESS: i32 = 0;
pub const INTERNAL: i32 = 1;
pub const PRECONDITION: i32 = 2;
pub const GIT: i32 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_match_error_contract() {
        assert_eq!(SUCCESS, 0);
        assert_eq!(INTERNAL, 1);
        assert_eq!(PRECONDITION, 2);
        assert_eq!(GIT, 3);
    }
}
