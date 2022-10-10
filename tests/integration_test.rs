use std::time::Duration;

use anyhow::Result;

use common::{k_apply, list_pods, retry_curl, setup_test, teardown_test};

mod common;

#[tokio::test]
async fn slight_test() -> Result<()> {
    let host_port = setup_test("slight-test").await?;

    let res = async {
        // apply slight workloads
        k_apply("deployments/workloads/slight").await?;

        // sleep for 30 seconds for the pods to be ready
        tokio::time::sleep(Duration::from_secs(30)).await;

        // check the test pod is running
        let cluster_name = format!("k3d-{}-{}", "slight-test", "cluster");
        list_pods(&cluster_name).await?;

        // curl for hello
        println!(" >>> curl http://localhost:{}/hello", host_port);
        retry_curl(&format!("http://localhost:{}/hello", host_port), 5, 10).await?;

        Ok(())
    }
    .await;

    teardown_test("slight-test").await?;
    res
}

#[tokio::test]
async fn spin_test() -> Result<()> {
    let host_port = setup_test("spin-test").await?;

    let res = async {
        // apply spin workloads
        k_apply("deployments/workloads/spin").await?;

        // sleep for 30 seconds for the pods to be ready
        tokio::time::sleep(Duration::from_secs(30)).await;

        // check the test pod is running
        let cluster_name = format!("k3d-{}-{}", "spin-test", "cluster");
        list_pods(&cluster_name).await?;

        // curl for hello
        println!(" >>> curl http://localhost:{}/hello", host_port);
        retry_curl(&format!("http://localhost:{}/hello", host_port), 1, 1).await?;

        Ok(())
    }
    .await;

    teardown_test("spin-test").await?;
    res
}

#[tokio::test]
async fn setup_idempotentcy() -> Result<()> {
    // FIXME: make setup and teardown idempotent

    // setup_test("setup-idempotentcy").await?;
    // setup_test("setup-idempotentcy").await?;

    // teardown_test("setup-idempotentcy").await?;
    Ok(())
}
