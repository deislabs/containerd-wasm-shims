use std::time::Duration;

use anyhow::Result;

use common::{k_apply, list_pods, random_payload, retry_get, retry_put, setup_test, teardown_test};

mod common;

static WORKLOAD_PATH: &str = "deployments/workloads";

static CLUSTER_SETUP_TIME: u64 = 30;
static RETRY_TIMES: u32 = 5;
static INTERVAL_IN_SECS: u64 = 10;

#[tokio::test]
async fn slight_test() -> Result<()> {
    let host_port = 8082;
    // check the test pod is running
    let cluster_name = format!("k3d-{}-{}", "test", "cluster");
    list_pods(&cluster_name).await?;

    // curl for hello
    println!(" >>> curl http://localhost:{}/slight/hello", host_port);
    let mut res = Vec::new();
    retry_get(
        &format!("http://localhost:{}/slight/hello", host_port),
        &mut res,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    println!("{}", String::from_utf8_lossy(&res));

    // put and get
    println!(
        " >>> curl -X PUT http://localhost:{}/slight/bar -d <value>",
        host_port
    );
    let payload = random_payload().await;
    let mut res = Vec::new();
    retry_put(
        &format!("http://localhost:{}/slight/bar", host_port),
        &payload,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    retry_get(
        &format!("http://localhost:{}/slight/foo", host_port),
        &mut res,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    assert_eq!(String::from_utf8_lossy(&res), payload);

    Ok(())
}

#[tokio::test]
async fn spin_test() -> Result<()> {
    let host_port = 8082;

    // check the test pod is running
    let cluster_name = format!("k3d-{}-{}", "test", "cluster");
    list_pods(&cluster_name).await?;

    // curl for hello
    println!(" >>> curl http://localhost:{}/spin/hello", host_port);
    let mut res = Vec::new();
    retry_get(
        &format!("http://localhost:{}/spin/hello", host_port),
        &mut res,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    println!("{}", String::from_utf8_lossy(&res));

    Ok(())
}
