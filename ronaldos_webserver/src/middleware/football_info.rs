use actix_web::http::{self};
use anyhow::{Context, Result};
use chrono::{serde::ts_seconds, DateTime, Utc};
use rustls::ClientConfig;
use rustls::RootCertStore;
use serde::{Deserialize, Serialize};
use simd_json::{json, owned::Value};
use simd_json::prelude::{ValueAsScalar, ValueAsContainer};
use std::{collections::HashMap, io::Write, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, info};

#[derive(Serialize, Deserialize, Clone)]
pub struct Fixture {
    pub fixture_id: u32,
    score: String,
    home: String,
    away: String,
    venue: String,
    #[serde(with = "ts_seconds")]
    timestamp: DateTime<Utc>,
}

pub struct FootballApi {
    /// map of league name as key and Fixture as item
    cache: RwLock<HashMap<String, Vec<Value>>>,
    url: http::uri::Uri,
    api_key: String,
    cert_store: Arc<RootCertStore>,
}

impl FootballApi {
    pub async fn new(
        season: &str,
        team: &str,
        api_key: String,
        cert_store: Arc<RootCertStore>,
    ) -> Self {
        let api_uri = http::Uri::from_str(&format!(
            "https://v3.football.api-sports.io/fixtures?season={}&team={}",
            season, team
        ))
        .unwrap();

        FootballApi {
            cache: RwLock::new(HashMap::new()),
            url: api_uri,
            api_key,
            cert_store,
        }
    }

    pub async fn fixtures<T: Write>(&self, writer: &mut T) -> Result<()> {
        let mut cache = self.cache.read().await;

        if cache.is_empty() {
            if self.api_key.is_empty() {
                info!("no football api key set. omitting fixture data");
                return Ok(());
            }

            debug!("cache not loaded yet, sending football request");
            let raw = self.football_api_request().await?;
            let map = to_data_model(raw).await?;

            // relock as write
            drop(cache);
            self.cache.write().await.extend(map);
            cache = self.cache.read().await;
        }

        Ok(simd_json::to_writer(writer, &*cache)?)
    }

    async fn football_api_request(&self) -> anyhow::Result<Value> {
        debug!("downloading match data from football-api");
        let config = ClientConfig::builder()
            .with_root_certificates(self.cert_store.clone())
            .with_no_client_auth();
        let client = awc::Client::builder()
            .connector(awc::Connector::new().rustls_0_23(Arc::new(config)))
            .finish();
        let request = client
            .get(&self.url)
            .insert_header(("X-RapidAPI-Host", "api-football-v2.p.rapidapi.com"))
            .insert_header(("X-RapidAPI-Key", self.api_key.as_str()));
        let mut res = request.send().await.unwrap();
        res.json::<Value>()
            .await
            .context("not a valid json reponse body")
    }
}

async fn to_data_model(json: Value) -> Result<HashMap<String, Vec<Value>>> {
    let mut fixtures: HashMap<String, Vec<Value>> = HashMap::new();
    for fixt in json["response"]
        .as_array()
        .with_context(|| format!("response: {}", json))?
    {
        let goals_empty =
            fixt["goals"]["home"] == json!(null) || fixt["goals"]["away"] == json!(null);
        let score = if goals_empty {
            "".to_string()
        } else {
            format!("{} - {}", fixt["goals"]["home"], fixt["goals"]["away"])
        };

        let match_entry = json! {{
            "home" : fixt["teams"]["home"]["name"],
            "away" : fixt["teams"]["away"]["name"],
            "venue" : fixt["fixture"]["venue"]["name"],
            "score" : score,
            "timestamp" : fixt["fixture"]["timestamp"],
            "fixture_id" : fixt["fixture"]["id"],
        }};

        fixtures
            .entry(
                fixt["league"]["name"]
                    .as_str()
                    .expect("need a key")
                    .to_string(),
            )
            .or_default()
            .push(match_entry);
    }
    Ok(fixtures)
}
