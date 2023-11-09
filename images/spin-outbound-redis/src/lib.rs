use anyhow::{anyhow, Context, Result};
use spin_sdk::{
    http::responses::internal_server_error,
    http::{IntoResponse, Request, Response},
    http_component, redis,
};


const REDIS_ADDRESS_ENV: &str = "REDIS_ADDRESS";
const REDIS_CHANNEL_ENV: &str = "REDIS_CHANNEL";

#[http_component]
fn hello_world(_req: Request) -> Result<impl IntoResponse> {
    let address = std::env::var(REDIS_ADDRESS_ENV)?;
    let channel = std::env::var(REDIS_CHANNEL_ENV)?;

    let conn = redis::Connection::open(&address)?;

    // Get the message to publish from the Redis key "mykey"
    let payload = conn
        .get("mykey")
        .map_err(|_| anyhow!("Error querying Redis"))?
        .context("no value for key 'mykey'")?;

    // Set the Redis key "spin-example" to value "Eureka!"
    conn.set("spin-example", &"Eureka!".to_owned().into_bytes())
        .map_err(|_| anyhow!("Error executing Redis set command"))?;

    // Set the Redis key "int-key" to value 0
    conn.set("int-key", &format!("{:x}", 0).into_bytes())
        .map_err(|_| anyhow!("Error executing Redis set command"))?;
    let int_value = conn
        .incr("int-key")
        .map_err(|_| anyhow!("Error executing Redis incr command",))?;
    assert_eq!(int_value, 1);

    // Publish to Redis
    match conn.publish(&channel, &payload) {
        Ok(()) => Ok(Response::new(200, ())),
        Err(_e) => Ok(internal_server_error()),
    }
}