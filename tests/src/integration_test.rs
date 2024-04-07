#[cfg(test)]
mod test {
    use tokio::process::Command;

    use crate::{random_payload, retry_get, retry_put};
    use anyhow::Result;

    const RETRY_TIMES: u32 = 5;
    const INTERVAL_IN_SECS: u64 = 10;

    #[tokio::test]
    async fn slight_test() -> Result<()> {
        let host_port = 8082;
        // curl for hello
        println!(" >>> curl http://localhost:{}/slight/hello", host_port);
        let res = retry_get(
            &format!("http://localhost:{}/slight/hello", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), "hello world!");

        // put and get
        println!(
            " >>> curl -X PUT http://localhost:{}/slight/set -d <value>",
            host_port
        );
        let payload = random_payload().await;
        retry_put(
            &format!("http://localhost:{}/slight/set", host_port),
            &payload,
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        let res = retry_get(
            &format!("http://localhost:{}/slight/get", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), payload);

        Ok(())
    }

    #[tokio::test]
    async fn wws_test() -> Result<()> {
        let host_port = 8082;

        // curl for hello
        println!(" >>> curl http://localhost:{}/wws/hello", host_port);
        let res = retry_get(
            &format!("http://localhost:{}/wws/hello", host_port),
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

        // curl for hello
        println!(" >>> curl http://localhost:{}/lunatic/hello", host_port);
        let res = retry_get(
            &format!("http://localhost:{}/lunatic/hello", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), "Hello :)");

        Ok(())
    }
}
