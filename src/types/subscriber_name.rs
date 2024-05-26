use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> Result<SubscriberName, String> {
        let name_forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];

        let empty = s.trim().is_empty();
        let too_long = s.graphemes(true).count() > 256;
        let forbidden_characters = s.chars().any(|c| name_forbidden_characters.contains(&c));

        if empty || too_long || forbidden_characters {
            return Err(format!("{} is not a valid subscriber name!", s));
        }

        Ok(SubscriberName(s))
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::{assert_err, assert_ok};

    #[test]
    fn test_name_max_length_256() {
        let name = "ё".repeat(256);
        assert_ok!(SubscriberName::parse(name));
        let name = "ё".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn test_whitespace_only_names_are_invalid() {
        let name = "   ".to_string();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn test_name_containing_forbidden_characters_is_invalid() {
        for c in &['/', '(', ')', '"', '<', '>', '\\', '{', '}'] {
            let name = c.to_string();
            assert_err!(SubscriberName::parse(name));
        }
    }

    #[test]
    fn test_valid_name_is_accepted() {
        let name = "Ursula Le Guin".to_string();
        assert_ok!(SubscriberName::parse(name));
    }
}
