use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::rest_json};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Response {
    #[serde(default)]
    email_identities: Vec<IdentityXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IdentityXml {
    identity_name: String,
    identity_type: Option<String>,
    sending_enabled: Option<bool>,
    verification_status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Identity {
    pub region: String,
    pub identity: String,
    pub identity_type: Option<String>,
    pub sending_enabled: Option<bool>,
    pub verification_status: Option<String>,
}

pub fn list_email_identities(client: &AwsClient, region: &str) -> Result<Vec<Identity>> {
    let endpoint = format!("email.{region}.amazonaws.com");
    let mut token: Option<String> = None;
    let mut output = Vec::new();
    loop {
        let query = token
            .iter()
            .map(|value| ("NextToken".into(), value.clone()))
            .collect();
        let response: Response = rest_json::get(
            client,
            "ses",
            region,
            &endpoint,
            "/v2/email/identities",
            query,
        )?;
        output.extend(
            response
                .email_identities
                .into_iter()
                .map(|identity| Identity {
                    region: region.into(),
                    identity: identity.identity_name,
                    identity_type: identity.identity_type,
                    sending_enabled: identity.sending_enabled,
                    verification_status: identity.verification_status,
                }),
        );
        token = response.next_token;
        if token.is_none() {
            break;
        }
    }
    Ok(output)
}
