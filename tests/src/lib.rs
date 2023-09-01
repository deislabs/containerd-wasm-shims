use std::{io::Read, time::Duration};

use anyhow::Result;
use curl::easy::Easy;
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
    let mut i = 0;
    let mut handle = Easy::new();
    handle.url(url)?;
    loop {
        let res = {
            let mut transfer = handle.transfer();
            transfer.write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()
        };
        let response_code = handle.response_code()?;
        // verify res is ok and not 404
        if res.is_ok() && response_code != 404 {
            break;
        }
        i += 1;
        if i == retry_times {
            anyhow::bail!("failed to curl for hello");
        }
        tokio::time::sleep(Duration::from_secs(interval_in_secs)).await;
    }
    Ok(())
}

pub async fn retry_put(
    url: &str,
    data: &str,
    retry_times: u32,
    interval_in_secs: u64,
) -> Result<()> {
    let mut i = 0;
    let mut handle = Easy::new();
    handle.url(url)?;
    handle.put(true)?;
    handle.post_field_size(data.len() as u64)?;
    loop {
        let res = {
            let mut transfer = handle.transfer();
            transfer.read_function(|into| Ok(data.as_bytes().read(into).unwrap_or(0)))?;
            transfer.write_function(|data| {
                println!("{}", String::from_utf8_lossy(data));
                Ok(data.len())
            })?;
            transfer.perform()
        };

        let response_code = handle.response_code()?;
        if res.is_ok() && response_code != 404 {
            break;
        }
        i += 1;
        if i == retry_times {
            anyhow::bail!("failed to curl for hello");
        }
        tokio::time::sleep(Duration::from_secs(interval_in_secs)).await;
    }
    Ok(())
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
