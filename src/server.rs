use std::{fmt::Debug, future::Future, net::SocketAddr, pin::Pin, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    body::Incoming,
    server::conn::http1,
    service::{service_fn, Service},
    Request, Response,
};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;

use crate::{RPCError, RPCRequest, RPCResponse};

#[async_trait]
pub trait Handle {
    type Request: for<'de> Deserialize<'de> + Send + Sync + Clone + 'static;
    type Response: Serialize + Send;

    async fn handle(
        &self,
        method: &str,
        req: Option<Self::Request>,
    ) -> std::result::Result<Option<Self::Response>, RPCError>;

    async fn batch_handle(
        &self,
        reqests: Vec<RPCRequest<Option<Self::Request>>>,
    ) -> Vec<RPCResponse<Self::Response>> {
        let mut response = vec![];
        for reqest in reqests {
            let resp = self
                .handle(&reqest.method, reqest.params)
                .await
                .map_or_else(
                    |e| RPCResponse::error(reqest.id.clone(), e),
                    |v| RPCResponse::result(reqest.id.clone(), v),
                );
            response.push(resp);
        }
        response
    }
}

async fn _handle<H>(req_body: serde_json::Value, handle: &H) -> Result<serde_json::Value>
where
    H: Handle,
    H::Request: Debug,
{
    let req: RPCRequest<Option<H::Request>> = serde_json::from_value(req_body)?;

    log::info!("Get call method: {}", &req.method);
    log::debug!("Params is: {:?}", &req.params);

    let r = match handle.handle(&req.method, req.params).await {
        Ok(v) => RPCResponse::result(req.id, v),
        Err(e) => RPCResponse::error(req.id, e),
    };

    r.into_value()
}
async fn _batch_handle<H>(req_body: serde_json::Value, handle: &H) -> Result<serde_json::Value>
where
    H: Handle + Sync,
    H::Request: Debug,
{
    let req: Vec<RPCRequest<Option<H::Request>>> = serde_json::from_value(req_body)?;

    log::debug!("Batch params is: {:?}", &req);

    let r = handle.batch_handle(req).await;

    Ok(serde_json::to_value(r)?)
}

struct HandleHttp<H> {
    handle: Arc<H>,
}

impl<H> Service<Request<Incoming>> for HandleHttp<H>
where
    H: Handle + Send + Sync + 'static,
    H::Request: Debug,
{
    type Response = Response<Full<Bytes>>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, request: Request<Incoming>) -> Self::Future {
        let handle = self.handle.clone();

        let r = async move {
            let req_body = request
                .into_body()
                .collect()
                .await
                .map_err(|e| anyhow!("{e}"))?
                .to_bytes()
                .to_vec();

            log::debug!("Request Body: {:?}", req_body);

            let req_body = serde_json::from_slice::<Value>(&req_body)?;

            let body = if req_body.is_object() {
                let r = _handle(req_body, handle.as_ref()).await?;
                serde_json::to_string(&r)?
            } else if req_body.is_array() {
                let r = _batch_handle(req_body, handle.as_ref()).await?;
                serde_json::to_string(&r)?
            } else {
                return Err(anyhow!("Unsupport type"));
            };
            log::debug!("Response Body: {:?}", body);

            let resp = Response::builder()
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(body)))?;

            Ok(resp)
        };

        Box::pin(r)
    }
}

pub async fn serve<H>(addr: &SocketAddr, handle: H) -> Result<()>
where
    H: Handle + Send + Sync + 'static,
    H::Request: Debug,
{
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    let handle = Arc::new(handle);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let handle = handle.clone();
        let service = service_fn(move |req| {
            let value = handle.clone();
            async move { HandleHttp { handle: value }.call(req).await }
        });

        if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
            println!("Error serving connection: {:?}", err);
        }
    }
}
