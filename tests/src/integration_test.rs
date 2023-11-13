#[cfg(test)]
mod test {
    use redis::AsyncCommands;
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
    async fn spin_test() -> Result<()> {
        let host_port = 8082;

        // curl for hello
        println!(" >>> curl http://localhost:{}/spin/hello", host_port);
        let res = retry_get(
            &format!("http://localhost:{}/spin/hello", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), "Hello world from Spin!");

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

    #[tokio::test]
    async fn spin_keyvalue_test() -> Result<()> {
        let host_port = 8082;

        // curl for hello
        println!(" >>> curl http://localhost:{}/keyvalue/keyvalue", host_port);
        let res = retry_get(
            &format!("http://localhost:{}/keyvalue/keyvalue", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;
        assert_eq!(String::from_utf8_lossy(&res), "wow");

        Ok(())
    }

    #[tokio::test]
    async fn spin_inbound_redis_outbound_redis_test() -> Result<()> {
        let host_port = 8082;
        let forward_port = 6380;
        let redis_port = 6379;

        // Ensure kubectl is in PATH
        if !is_kubectl_installed().await? {
            anyhow::bail!("kubectl is not installed");
        }

        port_forward_redis(forward_port, redis_port).await?;

        let client = redis::Client::open(format!("redis://localhost:{}", forward_port))?;
        let mut con = client.get_async_connection().await?;

        // curl for hello
        println!(
            " >>> curl http://localhost:{}/outboundredis/hello",
            host_port
        );
        let _ = retry_get(
            &format!("http://localhost:{}/outboundredis/hello", host_port),
            RETRY_TIMES,
            INTERVAL_IN_SECS,
        )
        .await?;

        // Retrieve the value for the key 'spin-example'
        let key: String = con.get("spin-example").await?;
        assert_eq!(key, "Eureka!");

        let key: String = con.get("int-key").await?;
        assert_eq!(key, "1");

        Ok(())
    }

    async fn is_kubectl_installed() -> anyhow::Result<bool> {
        let output: Result<std::process::Output, std::io::Error> = Command::new("kubectl")
            .arg("version")
            .arg("--client")
            .output()
            .await;

        match output {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }

    async fn port_forward_redis(forward_port: u16, redis_port: u16) -> Result<()> {
        println!(
            " >>> kubectl portforward redis {}:{} ",
            forward_port, redis_port
        );
        Command::new("kubectl")
            .arg("port-forward")
            .arg("redis")
            .arg(format!("{}:{}", forward_port, redis_port))
            .output()
            .await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        Ok(())
    }
}
