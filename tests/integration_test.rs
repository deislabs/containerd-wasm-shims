use std::time::Duration;

use anyhow::Result;

use common::{k_apply, list_pods, random_payload, retry_get, retry_put, setup_test, teardown_test};

mod common;

static SLIGHT_WORKLOAD_PATH: &str = "deployments/workloads/slight";
static SPIN_WORKLOAD_PATH: &str = "deployments/workloads/spin";

static CLUSTER_SETUP_TIME: u64 = 30;
static RETRY_TIMES: u32 = 5;
static INTERVAL_IN_SECS: u64 = 10;

#[tokio::test]
async fn slight_test() -> Result<()> {
    let host_port = setup_test("slight-test").await?;

    let res = async {
        // apply slight workloads
        k_apply(SLIGHT_WORKLOAD_PATH).await?;

        // sleep for the pods to be ready
        tokio::time::sleep(Duration::from_secs(CLUSTER_SETUP_TIME)).await;

        // check the test pod is running
        let cluster_name = format!("k3d-{}-{}", "slight-test", "cluster");
        list_pods(&cluster_name).await?;

        // curl for hello
        println!(" >>> curl http://localhost:{}/hello", host_port);
        let mut res = Vec::new();
        retry_get(
            &format!("http://localhost:{}/hello", host_port),
            &mut res,
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        println!("{}", String::from_utf8_lossy(&res));

        // put and get
        println!(
            " >>> curl -X PUT http://localhost:{}/bar -d <value>",
            host_port
        );
        let payload = random_payload().await;
        let mut res = Vec::new();
        retry_put(
            &format!("http://localhost:{}/bar", host_port),
            &payload,
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        retry_get(
            &format!("http://localhost:{}/foo", host_port),
            &mut res,
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), payload);

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
        k_apply(SPIN_WORKLOAD_PATH).await?;

        // sleep for 30 seconds for the pods to be ready
        tokio::time::sleep(Duration::from_secs(CLUSTER_SETUP_TIME)).await;

        // check the test pod is running
        let cluster_name = format!("k3d-{}-{}", "spin-test", "cluster");
        list_pods(&cluster_name).await?;

        // curl for hello
        println!(" >>> curl http://localhost:{}/hello", host_port);
        let mut res = Vec::new();
        retry_get(
            &format!("http://localhost:{}/hello", host_port),
            &mut res,
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        println!("{}", String::from_utf8_lossy(&res));

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
