use anyhow::Result;
use spin_sdk::{
    http::{Request, Response},
    http_component,
};

#[http_component]
fn hello_world(_req: Request) -> Result<Response> {
    println!("Hello, world! You should see me in pod logs");
    Ok(http::Response::builder()
        .status(200)
        .body(Some("Hello world from Spin!".into()))?)
}