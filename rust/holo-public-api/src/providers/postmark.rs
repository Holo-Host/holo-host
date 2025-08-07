/// send an email using postmark template
pub async fn send_email(
    postmark_api_key: String,
    to: String,
    template_alias: String,
    data: bson::Document,
) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let response = match client
        .post("https://api.postmarkapp.com/email/withTemplate")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("X-Postmark-Server-Token", postmark_api_key)
        .body(
            bson::doc! {
                "From": "no-reply@holo.host".to_string(),
                "To": to,
                "TemplateAlias": template_alias,
                "TemplateModel": data,
            }
            .to_string(),
        )
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            tracing::error!("failed to send email: {}", err);
            return Err(err);
        }
    };
    if response.status() != 200 {
        tracing::error!("failed to send email: {}", response.text().await.unwrap());
    }

    Ok(())
}
