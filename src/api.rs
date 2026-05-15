// API helper module
//
// Responsibilities:
// - Scrape Fast.com to acquire a short-lived token used by Netflix's
//   speedtest API
// - Request a list of test server URLs to be used by download workers
//
// This module intentionally keeps data types minimal (only the fields
// returned/used by the application are modelled).
use anyhow::{Result, anyhow};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ApiResponse {
    pub targets: Vec<Server>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub url: String,
}

pub async fn fetch_token(client: &Client) -> Result<String> {
    // Fetch a temporary token from fast.com.
    //
    // Steps:
    // 1. GET https://fast.com and locate the JS bundle name
    // 2. Fetch that JS bundle and extract the token from the bundle
    //
    // Parameters
    // - `client`: a configured reqwest `Client` (user-agent set)
    //
    // Returns: the token string used when requesting test servers.
    let html = client.get("https://fast.com").send().await?.text().await?;

    let js_re = Regex::new(r#"app-.*?\.js"#)?;

    let js_bundle = js_re
        .find(&html)
        .ok_or_else(|| anyhow!("Could not locate Fast.com JS bundle"))?
        .as_str();

    let js_url = format!("https://fast.com/{}", js_bundle);

    let bundle = client.get(js_url).send().await?.text().await?;

    let token_re = Regex::new(r#"token:"(.*?)""#)?;

    let token = token_re
        .captures(&bundle)
        .and_then(|c| c.get(1))
        .ok_or_else(|| anyhow!("Could not extract token"))?
        .as_str();

    Ok(token.to_string())
}

pub async fn fetch_servers(
    client: &Client,
    token: &str,
    connections: usize,
) -> Result<Vec<Server>> {
    // Request a list of test server endpoints from Fast.com's API.
    //
    // Parameters
    // - `client`: HTTP client
    // - `token`: token retrieved from `fetch_token`
    // - `connections`: number of desired server URLs (urlCount)
    //
    // Returns a Vec of `Server` records. Returns an error if no targets
    // are returned.
    let url = format!(
        "https://api.fast.com/netflix/speedtest/v2\
        ?https=true\
        &token={}\
        &urlCount={}",
        token, connections
    );

    let response = client.get(url).send().await?.json::<ApiResponse>().await?;

    if response.targets.is_empty() {
        return Err(anyhow!("No test servers returned"));
    }

    Ok(response.targets)
}
