use crate::routes::QueryParameters;

#[derive(Debug)]
pub struct SubscriptionToken(String);

impl SubscriptionToken {
    pub fn parse(token: String) -> Result<SubscriptionToken, String> {
        if (token.len() != 25) || !token.chars().all(|c| c.is_alphanumeric()) {
            return Err(format!("{} is not a valid token!", token));
        }

        Ok(SubscriptionToken(token))
    }
}

impl AsRef<str> for SubscriptionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryInto<SubscriptionToken> for QueryParameters {
    type Error = String;

    fn try_into(self) -> Result<SubscriptionToken, Self::Error> {
        let token = SubscriptionToken::parse(self.subscription_token)?;
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_ok};

    use super::*;

    #[test]
    fn test_subscription_token_length_25() {
        let token = "a".repeat(25);
        assert_ok!(SubscriptionToken::parse(token));
        let token = "b".repeat(26);
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn test_subscription_token_with_non_alphanumeric_characters_invalid() {
        let token = "?".repeat(25);
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn test_valid_token_accepted() {
        let token = "Ujn0892U354jhlksdFNbkljsd".to_string();
        assert_ok!(SubscriptionToken::parse(token));
    }
}
