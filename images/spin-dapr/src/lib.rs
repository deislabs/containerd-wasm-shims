use anyhow::{anyhow, Result};
use spin_sdk::{
    http::{Request, Response},
    http_component, redis,
};

// Expect redis running on localhost or in the same pod
const ADDRESS: &str = "redis://localhost:6379";
const KEY: &str = "lastEvent";


#[http_component]
fn handle_spin_dapr(req: Request) -> Result<Response> {
    println!("{:?}\n", req);

    if req.uri().path().ends_with("kevents"){
        let value = req.body().clone().unwrap();
        println!("Set: {:?}", value);
        redis::set(ADDRESS, KEY, &value)
        .map_err(|_| anyhow!("Error executing Redis set command"))?;
    }

    let value = redis::get(ADDRESS, KEY)
        .map_err(|_| anyhow!("Error executing Redis get command"))?;

    Ok(http::Response::builder()
        .status(200)
        .header("foo", "bar")
        .body(Some(value.into()))?)
}
