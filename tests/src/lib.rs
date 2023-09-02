use std::time::Duration;

use anyhow::Result;
use http::StatusCode;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ListParams, config::KubeConfigOptions, Api, Client, Config, ResourceExt};
use rand::{distributions::Alphanumeric, Rng};

mod integration_test;

pub async fn retry_get(
    url: &str,
    buf: &mut Vec<u8>,
    retry_times: u32,
    interval_in_secs: u64,
) -> Result<()> {
    for i in 1..=retry_times {
        println!("GETting data from {url}");
        let client = reqwest::Client::new();
        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                println!("GETted data from {url}, response_code: {status}, text {text}");
                if status != StatusCode::NOT_FOUND {
                    *buf = text.as_bytes().to_vec();
                    return Ok(())
                }
            }
            Err(err) => {
                println!("error GETting data from {url}, response_code: {err}");
            }
        }
        if i < retry_times {
            tokio::time::sleep(Duration::from_secs(interval_in_secs)).await;
        }
    }
    anyhow::bail!("failed to curl for {}", url);
}

pub async fn retry_put(
    url: &str,
    data: &str,
    retry_times: u32,
    interval_in_secs: u64,
) -> Result<()> {
    for i in 1..=retry_times {
        println!("PUTting data to {url}: {data}");
        let client = reqwest::Client::new();
        match client.put(url).body(data.to_owned()).send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                println!("PUTted data to {url}, response_code: {status}, text {text}");
                if status != StatusCode::NOT_FOUND {
                    return Ok(());
                }
            }
            Err(err) => {
                println!("error PUTting data to {url}, response_code: {err}");
            }
        }
        if i < retry_times {
            tokio::time::sleep(Duration::from_secs(interval_in_secs)).await;
        }
    }
    anyhow::bail!("failed to curl for {}", url);
}

pub async fn list_pods(cluster_name: &str) -> Result<()> {
    let config = Config::from_kubeconfig(&KubeConfigOptions {
        context: Some(cluster_name.to_string()),
        ..Default::default()
    })
    .await?;

    let client = Client::try_from(config)?;

    let pods: Api<Pod> = Api::default_namespaced(client);
    for p in pods.list(&ListParams::default()).await? {
        println!("found pod {}", p.name_any());
    }
    Ok(())
}

pub async fn random_payload() -> String {
    let rng = rand::thread_rng();
    let payload: String = rng
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    payload
}
