use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::query};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Response {
    get_caller_identity_result: Identity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Identity {
    pub account: String,
    pub arn: String,
    pub user_id: String,
}

pub fn get_caller_identity(client: &AwsClient, region: &str) -> Result<Identity> {
    let endpoint = format!("sts.{region}.amazonaws.com");
    let response: Response = query::call(
        client,
        &endpoint,
        "sts",
        region,
        "GetCallerIdentity",
        "2011-06-15",
        &[],
    )?;
    Ok(response.get_caller_identity_result)
}
