/// Generate a new random UUIDv4 identifier as a lowercase hyphenated string.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_valid_uuidv4() {
        let id = new_id();
        let parsed = uuid::Uuid::parse_str(&id).expect("should be a valid UUID string");
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn new_id_is_unique() {
        assert_ne!(new_id(), new_id());
    }
}
