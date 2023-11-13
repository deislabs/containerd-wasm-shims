use anyhow::Result;
use spin_sdk::{
    http::{Request, Response},
    http_component,
    key_value::Store,
};

/// A simple Spin HTTP component.
#[http_component]
fn handle_kv(_req: Request) -> Result<Response> {
    let store = Store::open("foo")?;
    store.set("mykey", "wow".as_bytes())?;
    let value = store.get("mykey")?;
    Ok(Response::builder()
        .status(200)
        .body(value)
        .build())
}