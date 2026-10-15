use chrono::{Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{AwsClient, Result, protocols::json};

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct Request {
    time_period: TimePeriod,
    granularity: &'static str,
    metrics: Vec<&'static str>,
    group_by: Vec<GroupBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_page_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct TimePeriod {
    start: String,
    end: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct GroupBy {
    r#type: &'static str,
    key: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Response {
    #[serde(default)]
    pub results_by_time: Vec<ResultByTime>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResultByTime {
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Group {
    #[serde(default)]
    pub keys: Vec<String>,
    pub metrics: Metrics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Metrics {
    pub unblended_cost: Amount,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Amount {
    pub amount: String,
    pub unit: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CostReport {
    pub start: String,
    pub end: String,
    pub by_service: Response,
    pub by_region: Response,
}

pub fn month_to_date(client: &AwsClient) -> Result<CostReport> {
    let today = Utc::now().date_naive();
    let start = today.with_day(1).expect("day one exists").to_string();
    let end = (today + Duration::days(1)).to_string();
    let by_service = get(client, &start, &end, "SERVICE")?;
    let by_region = get(client, &start, &end, "REGION")?;
    Ok(CostReport {
        start,
        end,
        by_service,
        by_region,
    })
}

fn get(client: &AwsClient, start: &str, end: &str, key: &str) -> Result<Response> {
    let mut output = Response {
        results_by_time: Vec::new(),
        next_page_token: None,
    };
    let mut token = None;
    loop {
        let response: Response = json::call(
            client,
            "ce",
            "us-east-1",
            "ce.us-east-1.amazonaws.com",
            "AWSInsightsIndexService.GetCostAndUsage",
            &Request {
                time_period: TimePeriod {
                    start: start.into(),
                    end: end.into(),
                },
                granularity: "MONTHLY",
                metrics: vec!["UnblendedCost"],
                group_by: vec![GroupBy {
                    r#type: "DIMENSION",
                    key: key.into(),
                }],
                next_page_token: token,
            },
        )?;
        output.results_by_time.extend(response.results_by_time);
        token = response.next_page_token;
        if token.is_none() {
            break;
        }
    }
    Ok(output)
}
