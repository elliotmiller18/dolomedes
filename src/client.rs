/*
    we'll have:
    ping (self explanatory)
    find_node (returns k closest nodes)
    find_value (returns k closest nodes or value if it's stored here)
    store (upload a file)
*/

use std::net::TcpStream;
use anyhow::Result;

pub fn run() -> Result<()> {

    let stream = TcpStream::connect("127.0.0.1:8080")?;

    Ok(())
}