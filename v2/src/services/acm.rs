use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::json};

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListResponse {
    #[serde(default)]
    certificate_summary_list: Vec<CertificateXml>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CertificateXml {
    domain_name: Option<String>,
    status: Option<String>,
    r#type: Option<String>,
    in_use: Option<bool>,
    not_after: Option<f64>,
    #[serde(default)]
    subject_alternative_name_summaries: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Certificate {
    pub region: String,
    pub domain_name: Option<String>,
    pub status: Option<String>,
    pub certificate_type: Option<String>,
    pub in_use: Option<bool>,
    pub not_after: Option<String>,
    pub subject_alternative_names: Vec<String>,
}

pub fn list_certificates(client: &AwsClient, region: &str) -> Result<Vec<Certificate>> {
    let endpoint = format!("acm.{region}.amazonaws.com");
    let mut token = None;
    let mut output = Vec::new();
    loop {
        let response: ListResponse = json::call(
            client,
            "acm",
            region,
            &endpoint,
            "CertificateManager.ListCertificates",
            &ListRequest { next_token: token },
        )?;
        output.extend(response.certificate_summary_list.into_iter().map(|cert| {
            Certificate {
                region: region.into(),
                domain_name: cert.domain_name,
                status: cert.status,
                certificate_type: cert.r#type,
                in_use: cert.in_use,
                not_after: cert
                    .not_after
                    .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds as i64, 0))
                    .map(|date| date.format("%Y-%m-%d").to_string()),
                subject_alternative_names: cert.subject_alternative_name_summaries,
            }
        }));
        token = response.next_token;
        if token.is_none() {
            break;
        }
    }
    Ok(output)
}
