use anyhow::Result;
use bytes::Bytes;
use spin_sdk::redis_component;
use std::str::from_utf8;
use spin_sdk::{redis, variables};

/// A simple Spin Redis component.
#[redis_component]
fn on_message(message: Bytes) -> Result<()> {
    
    let address = variables::get("redis_address").expect("could not get variable");
    let channel = variables::get("redis_channel").expect("could not get variable");
    let conn = redis::Connection::open(&address)?;

    println!("{}", from_utf8(&message)?);
    
    // Publish to Redis
    conn.publish(&channel, &message.to_vec())?;

    Ok(())
}
