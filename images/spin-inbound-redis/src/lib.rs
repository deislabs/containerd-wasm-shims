use anyhow::Result;
use bytes::Bytes;
use spin_sdk::redis_component;
use std::str::from_utf8;
use spin_sdk::redis;

const REDIS_ADDRESS_ENV: &str = "REDIS_ADDRESS";
const REDIS_CHANNEL_ENV: &str = "REDIS_CHANNEL";

/// A simple Spin Redis component.
#[redis_component]
fn on_message(message: Bytes) -> Result<()> {
    
    let address = std::env::var(REDIS_ADDRESS_ENV)?;
    let channel = std::env::var(REDIS_CHANNEL_ENV)?;

    let conn = redis::Connection::open(&address)?;

    println!("{}", from_utf8(&message)?);
    
    // Publish to Redis
    conn.publish(&channel, &message.to_vec())?;

    Ok(())
}
