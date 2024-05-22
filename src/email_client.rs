use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};

use crate::types::SubscriberEmail;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: SecretString,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct EmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: SecretString,
        http_client_timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(http_client_timeout)
            .build()
            .expect("Unable to build HTTP client");
        Self {
            http_client,
            base_url,
            sender,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/email", self.base_url);
        let request_body = EmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };
        let _ = self
            .http_client
            .post(&url)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use secrecy::Secret;
    use wiremock::{
        matchers::{any, header, header_exists, method, path},
        Match, Mock, MockServer, ResponseTemplate,
    };

    use super::*;

    struct EmailBodyMatcher;

    impl Match for EmailBodyMatcher {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }

    fn get_subject() -> String {
        Sentence(1..2).fake()
    }

    fn get_content() -> String {
        Paragraph(1..10).fake()
    }

    fn get_email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn get_email_client(base_url: String) -> EmailClient {
        EmailClient::new(
            base_url,
            get_email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(200),
        )
    }

    #[tokio::test]
    async fn test_email_client_fires_expected_http_request() {
        let server = MockServer::start().await;

        let email_client = get_email_client(server.uri());

        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            .and(EmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        // Sending the actual email towards the mock server
        let _ = email_client
            .send_email(get_email(), &get_subject(), &get_content(), &get_content())
            .await;

        // Once the mock server goes out of scope, it iterates and asserts all the Mocks
        // Ours expects to receive exactly one request, and that's the assertion that is gonna do
        // If this does not happen, the test fails
    }

    #[tokio::test]
    async fn test_send_email_works_on_200_response() {
        let server = MockServer::start().await;

        let email_client = get_email_client(server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        // Sending the actual email towards the mock server
        let response = email_client
            .send_email(get_email(), &get_subject(), &get_content(), &get_content())
            .await;

        claims::assert_ok!(response);
    }

    #[tokio::test]
    async fn test_send_email_fails_on_no_200_response() {
        let server = MockServer::start().await;

        let email_client = get_email_client(server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        // Sending the actual email towards the mock server
        let response = email_client
            .send_email(get_email(), &get_subject(), &get_content(), &get_content())
            .await;

        claims::assert_err!(response);
    }

    #[tokio::test]
    async fn test_send_email_fails_on_timeout() {
        let server = MockServer::start().await;

        let email_client = get_email_client(server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(180)))
            .expect(1)
            .mount(&server)
            .await;

        // Sending the actual email towards the mock server
        let response = email_client
            .send_email(get_email(), &get_subject(), &get_content(), &get_content())
            .await;

        claims::assert_err!(response);
    }
}
