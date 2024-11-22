#![deny(warnings, unused_crate_dependencies)]

use anyhow::Result;
use json_rpc_server::call;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let value = json!([10, true]);
    let ret = call::<Value, Value>("http://127.0.0.1:8080", "example_fn1", &value, None)
        .await
        .unwrap();
    println!("{:?}", ret);

    let value = json!([100,]);
    let ret = call::<Value, Value>("http://127.0.0.1:8080", "example_fn2", &value, None)
        .await
        .unwrap();
    println!("{:?}", ret);

    Ok(())
}
