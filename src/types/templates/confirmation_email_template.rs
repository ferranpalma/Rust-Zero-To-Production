use askama_actix::Template;

#[derive(Template)]
#[template(path = "confirmation_email.html")]
pub struct ConfirmationEmailTemplate<'a> {
    pub confirmation_link: &'a str,
}
