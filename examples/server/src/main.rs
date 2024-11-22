#![deny(warnings, unused_crate_dependencies)]

use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use json_rpc_server::{serve, Handle, RPCError, RPCResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct ExampleHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExampleRequest {
    Param1((u32, bool)),
    Param2((u64,)),
    Other(serde_json::Value),
}

macro_rules! define_into {
    ($func: ident, $ret: ty, $e: ident) => {
        pub fn $func(self) -> RPCResult<$ret> {
            match self {
                Self::$e(v) => Ok(v),
                _ => Err(RPCError::invalid_params()),
            }
        }
    };
}

impl ExampleRequest {
    define_into!(into_param1, (u32, bool), Param1);
    define_into!(into_param2, (u64,), Param2);
}

#[async_trait]
impl Handle for ExampleHandle {
    type Request = ExampleRequest;
    type Response = Value;

    async fn handle(
        &self,
        method: &str,
        req: Option<Self::Request>,
    ) -> std::result::Result<Option<Self::Response>, RPCError> {
        match method {
            "example_fn1" => {
                let param = req
                    .clone()
                    .ok_or(RPCError::invalid_params())
                    .and_then(|v| v.into_param1())?;
                serde_json::to_value(param)
                    .map(|v| Some(v))
                    .map_err(|e| RPCError::internal_error(format!("{}", e)))
            }
            "example_fn2" => {
                let param = req
                    .clone()
                    .ok_or(RPCError::invalid_params())
                    .and_then(|v| v.into_param2())?;
                serde_json::to_value(param)
                    .map(|v| Some(v))
                    .map_err(|e| RPCError::internal_error(format!("{}", e)))
            }

            _ => Err(RPCError::unknown_method()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "full");
    }
    env_logger::init();
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let handle = ExampleHandle;
    serve(&addr, handle).await
}
