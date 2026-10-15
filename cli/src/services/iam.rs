use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{AwsClient, Error, Result, protocols::query};

use super::common::Members;

const ENDPOINT: &str = "iam.amazonaws.com";
const VERSION: &str = "2010-05-08";

fn call<T: DeserializeOwned>(
    client: &AwsClient,
    operation: &str,
    parameters: &[(String, String)],
) -> Result<T> {
    query::call(
        client,
        ENDPOINT,
        "iam",
        "us-east-1",
        operation,
        VERSION,
        parameters,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AliasesResponse {
    list_account_aliases_result: AliasResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AliasResult {
    #[serde(default)]
    account_aliases: Members<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SummaryResponse {
    get_account_summary_result: SummaryResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SummaryResult {
    #[serde(default)]
    summary_map: Entries,
}

#[derive(Debug, Default, Deserialize)]
struct Entries {
    #[serde(rename = "entry", default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    key: String,
    value: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UsersResponse {
    list_users_result: UsersResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UsersResult {
    #[serde(default)]
    users: Members<UserXml>,
    marker: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserXml {
    user_name: String,
    arn: String,
    create_date: Option<String>,
    password_last_used: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccessKeysResponse {
    list_access_keys_result: AccessKeysResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccessKeysResult {
    #[serde(default)]
    access_key_metadata: Members<AccessKey>,
    marker: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AccessKey {
    pub access_key_id: String,
    pub status: String,
    pub create_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MfaResponse {
    #[serde(rename = "ListMFADevicesResult")]
    result: MfaResult,
}

#[derive(Debug, Deserialize)]
struct MfaResult {
    #[serde(rename = "MFADevices", default)]
    devices: Members<MfaDevice>,
    #[serde(rename = "Marker")]
    marker: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MfaDevice {
    pub serial_number: String,
    pub enable_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GroupsResponse {
    list_groups_for_user_result: GroupsResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GroupsResult {
    #[serde(default)]
    groups: Members<GroupXml>,
    marker: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GroupXml {
    group_name: String,
    arn: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PoliciesResponse {
    list_attached_group_policies_result: PoliciesResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PoliciesResult {
    #[serde(default)]
    attached_policies: Members<Policy>,
    marker: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Policy {
    pub policy_name: String,
    pub policy_arn: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LoginProfileResponse {
    get_login_profile_result: LoginProfileResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LoginProfileResult {
    login_profile: LoginProfile,
}

#[derive(Debug, Deserialize)]
struct LoginProfile {
    #[serde(rename = "UserName")]
    _user_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct IamAudit {
    pub summary: BTreeMap<String, u64>,
    pub users: Vec<User>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    pub user_name: String,
    pub arn: String,
    pub create_date: Option<String>,
    pub password_last_used: Option<String>,
    pub console_password: bool,
    pub access_keys: Vec<AccessKey>,
    pub mfa_devices: Vec<MfaDevice>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Group {
    pub group_name: String,
    pub arn: String,
    pub attached_policies: Vec<Policy>,
}

pub fn list_account_aliases(client: &AwsClient) -> Result<Vec<String>> {
    let response: AliasesResponse = call(client, "ListAccountAliases", &[])?;
    Ok(response.list_account_aliases_result.account_aliases.members)
}

pub fn audit(client: &AwsClient) -> Result<IamAudit> {
    let response: SummaryResponse = call(client, "GetAccountSummary", &[])?;
    let summary = response
        .get_account_summary_result
        .summary_map
        .entries
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect();
    let mut marker: Option<String> = None;
    let mut source_users = Vec::new();
    loop {
        let params = marker
            .iter()
            .map(|value| ("Marker".into(), value.clone()))
            .collect::<Vec<_>>();
        let response: UsersResponse = call(client, "ListUsers", &params)?;
        source_users.extend(response.list_users_result.users.members);
        marker = response.list_users_result.marker;
        if marker.is_none() {
            break;
        }
    }
    let mut users = Vec::new();
    for user in source_users {
        let name = user.user_name.clone();
        users.push(User {
            user_name: user.user_name,
            arn: user.arn,
            create_date: user.create_date,
            password_last_used: user.password_last_used,
            console_password: has_login_profile(client, &name)?,
            access_keys: list_access_keys(client, &name)?,
            mfa_devices: list_mfa_devices(client, &name)?,
            groups: list_groups(client, &name)?,
        });
    }
    Ok(IamAudit { summary, users })
}

fn has_login_profile(client: &AwsClient, user: &str) -> Result<bool> {
    match call::<LoginProfileResponse>(
        client,
        "GetLoginProfile",
        &[("UserName".into(), user.into())],
    ) {
        Ok(response) => {
            let _ = response.get_login_profile_result.login_profile;
            Ok(true)
        }
        Err(Error::Aws { code, .. }) if code == "NoSuchEntity" => Ok(false),
        Err(error) => Err(error),
    }
}

fn list_access_keys(client: &AwsClient, user: &str) -> Result<Vec<AccessKey>> {
    let mut marker: Option<String> = None;
    let mut output = Vec::new();
    loop {
        let mut params = vec![("UserName".into(), user.into())];
        if let Some(value) = marker {
            params.push(("Marker".into(), value));
        }
        let response: AccessKeysResponse = call(client, "ListAccessKeys", &params)?;
        output.extend(response.list_access_keys_result.access_key_metadata.members);
        marker = response.list_access_keys_result.marker;
        if marker.is_none() {
            break;
        }
    }
    Ok(output)
}

fn list_mfa_devices(client: &AwsClient, user: &str) -> Result<Vec<MfaDevice>> {
    let mut marker: Option<String> = None;
    let mut output = Vec::new();
    loop {
        let mut params = vec![("UserName".into(), user.into())];
        if let Some(value) = marker {
            params.push(("Marker".into(), value));
        }
        let response: MfaResponse = call(client, "ListMFADevices", &params)?;
        output.extend(response.result.devices.members);
        marker = response.result.marker;
        if marker.is_none() {
            break;
        }
    }
    Ok(output)
}

fn list_groups(client: &AwsClient, user: &str) -> Result<Vec<Group>> {
    let mut marker: Option<String> = None;
    let mut source = Vec::new();
    loop {
        let mut params = vec![("UserName".into(), user.into())];
        if let Some(value) = marker {
            params.push(("Marker".into(), value));
        }
        let response: GroupsResponse = call(client, "ListGroupsForUser", &params)?;
        source.extend(response.list_groups_for_user_result.groups.members);
        marker = response.list_groups_for_user_result.marker;
        if marker.is_none() {
            break;
        }
    }
    let mut output = Vec::new();
    for group in source {
        let mut policies = Vec::new();
        marker = None;
        loop {
            let mut params = vec![("GroupName".into(), group.group_name.clone())];
            if let Some(value) = marker {
                params.push(("Marker".into(), value));
            }
            let response: PoliciesResponse = call(client, "ListAttachedGroupPolicies", &params)?;
            policies.extend(
                response
                    .list_attached_group_policies_result
                    .attached_policies
                    .members,
            );
            marker = response.list_attached_group_policies_result.marker;
            if marker.is_none() {
                break;
            }
        }
        output.push(Group {
            group_name: group.group_name,
            arn: group.arn,
            attached_policies: policies,
        });
    }
    Ok(output)
}
