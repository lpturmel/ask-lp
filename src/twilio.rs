pub async fn send_sms(http: &reqwest::Client, body: &str) -> Result<(), reqwest::Error> {
    let twilio_account_sid = std::env::var("TWILIO_ACCOUNT_SID").unwrap();
    let twilio_auth_token = std::env::var("TWILIO_AUTH_TOKEN").unwrap();
    let twilio_msg_service = std::env::var("TWILIO_MESSAGE_SERVICE_SID").unwrap();
    let twilio_to = std::env::var("TWILIO_TO").unwrap();

    let res = http
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            twilio_account_sid
        ))
        .basic_auth(twilio_account_sid, Some(twilio_auth_token))
        .form(&[
            ("MessagingServiceSid", twilio_msg_service),
            ("To", twilio_to),
            ("Body", body.to_string()),
        ])
        .send()
        .await?;

    let text = res.text().await?;
    println!("{}", text);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_sms() {
        dotenv::dotenv().ok();
        let http = reqwest::Client::new();

        let body = "Hello, world!";
        let res = send_sms(&http, body).await;
        assert!(res.is_ok());
    }
}
