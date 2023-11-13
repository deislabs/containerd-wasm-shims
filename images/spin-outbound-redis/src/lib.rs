use anyhow::{anyhow, Context, Result};
use spin_sdk::{
    http::responses::internal_server_error,
    http::{IntoResponse, Request, Response},
    http_component, redis,
    variables,
};

#[http_component]
fn hello_world(_req: Request) -> Result<impl IntoResponse> {
    let address = variables::get("redis_address").expect("could not get variable");
    let channel = variables::get("redis_channel").expect("could not get variable");

    let conn = redis::Connection::open(&address)?;

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

    // Get the Redis key "spin-example"
    let payload = conn
        .get("spin-example")
        .map_err(|_| anyhow!("Error querying Redis"))?
        .context("no value for key 'mykey'")?;

    // Publish to Redis
    match conn.publish(&channel, &payload) {
        Ok(()) => Ok(Response::new(200, ())),
        Err(_e) => Ok(internal_server_error()),
    }
}