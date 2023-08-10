use anyhow::Result;
use common::{list_pods, random_payload, retry_get, retry_put};
mod common;

const RETRY_TIMES: u32 = 5;
const INTERVAL_IN_SECS: u64 = 10;

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
        " >>> curl -X PUT http://localhost:{}/slight/set -d <value>",
        host_port
    );
    let payload = random_payload().await;
    let mut res = Vec::new();
    retry_put(
        &format!("http://localhost:{}/slight/set", host_port),
        &payload,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    retry_get(
        &format!("http://localhost:{}/slight/get", host_port),
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

#[tokio::test]
async fn wws_test() -> Result<()> {
    let host_port = 8082;

    // check the test pod is running
    let cluster_name = format!("k3d-{}-{}", "test", "cluster");
    list_pods(&cluster_name).await?;

    // curl for hello
    println!(" >>> curl http://localhost:{}/wws/hello", host_port);
    let mut res = Vec::new();
    retry_get(
        &format!("http://localhost:{}/wws/hello", host_port),
        &mut res,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    println!("{}", String::from_utf8_lossy(&res));

    Ok(())
}

#[tokio::test]
async fn lunatic_test() -> Result<()> {
    let host_port = 8082;

    // check the test pod is running
    let cluster_name = format!("k3d-{}-{}", "test", "cluster");
    list_pods(&cluster_name).await?;

    // curl for hello
    println!(" >>> curl http://localhost:{}/lunatic", host_port);
    let mut res = Vec::new();
    retry_get(
        &format!("http://localhost:{}/lunatic", host_port),
        &mut res,
        RETRY_TIMES,
        INTERVAL_IN_SECS,
    )
    .await?;
    println!("{}", String::from_utf8_lossy(&res));

    Ok(())
}
