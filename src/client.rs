use std::fmt::Debug;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{header::HeaderValue, Request, StatusCode, Uri};
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use serde::{Deserialize, Serialize};

use crate::{RPCError, RPCRequest, RPCResponse, RPCResult};

pub async fn call<P, R>(
    url: &str,
    method: &str,
    params: &P,
    auth: Option<&str>,
) -> RPCResult<Option<R>>
where
    R: for<'de> Deserialize<'de> + Debug,
    P: Serialize,
{
    let req = RPCRequest::new(method, params);
    let s = serde_json::to_string(&req).map_err(|e| RPCError::internal_error(format!("{e:?}")))?;
    let mut headers = vec![
        ("content-type", String::from("application/json")),
        ("User-Agent", String::from("hyper-client")),
    ];
    if let Some(t) = auth {
        let r = format!("Bearer {}", t);
        headers.push(("Authorization", r));
    }

    let (status_code, bytes) = http_post(url, s.as_bytes(), Some(&headers))
        .await
        .map_err(|e| RPCError::internal_error(format!("{e:?}")))?;

    if !status_code.is_success() {
        log::error!(
            "StatusCode:{:?}, Response is: {:?}",
            status_code,
            String::from_utf8_lossy(&bytes)
        );
        return Err(RPCError::internal_error(String::from(
            "Failed to request uri",
        )));
    } else {
        log::debug!(
            "StatusCode:{:?}, Response is: {:?}",
            status_code,
            String::from_utf8_lossy(&bytes)
        );
    }

    let resp: RPCResponse<R> =
        serde_json::from_slice(&bytes).map_err(|e| RPCError::internal_error(format!("{e:?}")))?;

    if let Some(e) = resp.error {
        Err(e)
    } else {
        Ok(resp.result)
    }
}

pub async fn batch_call<P, R>(
    url: &str,
    requests: &Vec<RPCRequest<P>>,
    auth: Option<&str>,
) -> Result<Vec<RPCResponse<R>>>
where
    R: for<'de> Deserialize<'de>,
    P: Serialize + Clone,
{
    let s = serde_json::to_string(&requests)?;

    let mut headers = vec![("content-type", String::from("application/json"))];
    if let Some(t) = auth {
        let r = format!("Bearer {}", t);
        headers.push(("Authorization", r));
    }

    let (status_code, bytes) = http_post(url, s.as_bytes(), Some(&headers)).await?;
    log::debug!(
        "StatusCode:{:?}, Response is: {:?}",
        status_code,
        String::from_utf8_lossy(&bytes)
    );

    if status_code.is_success() {
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        log::error!(
            "StatusCode:{:?}, Response is: {:?}",
            status_code,
            String::from_utf8_lossy(&bytes)
        );
        Err(anyhow!("Failed to request uri"))
    }
}

pub async fn http_post_ret_string(
    url: &str,
    body: &[u8],
    headers: Option<&[(&'static str, String)]>,
) -> Result<(StatusCode, String)> {
    http_post(url, body, headers)
        .await
        .map(|(code, msg)| (code, String::from_utf8_lossy(&msg).into_owned()))
}

pub async fn http_post(
    url: &str,
    body: &[u8],
    headers: Option<&[(&'static str, String)]>,
) -> Result<(StatusCode, Vec<u8>)> {
    let uri: Uri = url.parse()?;
    let request = Request::post(uri).body(Full::from(body.to_vec()))?;
    send_http_request(request, headers).await
}
pub async fn http_get_ret_string(
    url: &str,
    body: &[u8],
    headers: Option<&[(&'static str, String)]>,
) -> Result<(StatusCode, String)> {
    http_get(url, body, headers)
        .await
        .map(|(code, msg)| (code, String::from_utf8_lossy(&msg).into_owned()))
}

pub async fn http_get(
    url: &str,
    body: &[u8],
    headers: Option<&[(&'static str, String)]>,
) -> Result<(StatusCode, Vec<u8>)> {
    let uri: Uri = url.parse()?;
    let request = Request::get(uri).body(Full::from(body.to_vec()))?;
    send_http_request(request, headers).await
}

async fn send_http_request(
    mut request: Request<Full<Bytes>>,
    headers: Option<&[(&'static str, String)]>,
) -> Result<(StatusCode, Vec<u8>)> {
    let connector = HttpsConnector::new();
    let client = Client::builder(TokioExecutor::new()).build(connector);

    if let Some(v) = headers {
        let hs = request.headers_mut();
        for (h, v) in v.iter() {
            hs.insert(*h, HeaderValue::from_str(v)?);
        }
    }

    let response = client.request(request).await?;
    let status_code = response.status();
    let body = response.into_body().collect().await?.to_bytes().to_vec();
    Ok((status_code, body))
}
