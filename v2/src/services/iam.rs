use serde::Deserialize;

use crate::{AwsClient, Result, protocols::query};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Response {
    list_account_aliases_result: AliasResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AliasResult {
    #[serde(default)]
    account_aliases: Members,
}

#[derive(Debug, Default, Deserialize)]
struct Members {
    #[serde(rename = "member", default)]
    values: Vec<String>,
}

pub fn list_account_aliases(client: &AwsClient) -> Result<Vec<String>> {
    let response: Response = query::call(
        client,
        "iam.amazonaws.com",
        "iam",
        "us-east-1",
        "ListAccountAliases",
        "2010-05-08",
        &[],
    )?;
    Ok(response.list_account_aliases_result.account_aliases.values)
}
