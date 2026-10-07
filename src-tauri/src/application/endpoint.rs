use super::AppError;
use std::{
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
};
use tokio::sync::{Semaphore, oneshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) address: Option<SocketAddr>,
}
impl Endpoint {
    pub fn parse(host: &str, port: u32) -> Result<Self, AppError> {
        let port = u16::try_from(port)
            .ok()
            .filter(|p| *p != 0)
            .ok_or(AppError::InvalidPort)?;
        if host.is_empty() || host.len() > 253 || !host.is_ascii() {
            return Err(AppError::InvalidHost);
        }
        let literal = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        if let Ok(ip) = literal.parse::<IpAddr>() {
            return Ok(Self {
                host: host.into(),
                port,
                address: Some(SocketAddr::new(ip, port)),
            });
        }
        let name = host.strip_suffix('.').unwrap_or(host);
        if !name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        }) {
            return Err(AppError::InvalidHost);
        }
        Ok(Self {
            host: host.into(),
            port,
            address: None,
        })
    }
}

type Lookup = Arc<
    dyn Fn(String, u16) -> Pin<Box<dyn Future<Output = io::Result<Vec<SocketAddr>>> + Send>>
        + Send
        + Sync,
>;
#[derive(Clone)]
pub(super) struct Resolver {
    slot: Arc<Semaphore>,
    lookup: Lookup,
}
impl Resolver {
    pub fn new() -> Self {
        Self {
            slot: Arc::new(Semaphore::new(1)),
            lookup: Arc::new(|host, port| {
                Box::pin(async move {
                    let addresses = tokio::net::lookup_host((host.as_str(), port)).await?;
                    Ok(unique(addresses))
                })
            }),
        }
    }
    #[cfg(test)]
    pub fn injected(lookup: Lookup) -> Self {
        Self {
            slot: Arc::new(Semaphore::new(1)),
            lookup,
        }
    }
    pub async fn resolve(&self, endpoint: Endpoint) -> Result<Vec<SocketAddr>, &'static str> {
        if let Some(address) = endpoint.address {
            return Ok(vec![address]);
        }
        let permit = self
            .slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| "A previous DNS lookup is still finishing. Try again shortly.")?;
        let lookup = self.lookup.clone();
        let (send, receive) = oneshot::channel();
        // OS DNS work cannot be forcibly cancelled. The slot lives with the lookup,
        // not its waiting connection attempt, preventing cancellation from flooding it.
        tokio::spawn(async move {
            let _permit = permit;
            let result = lookup(endpoint.host, endpoint.port)
                .await
                .map(|addresses| unique(addresses.into_iter()));
            let _ = send.send(result);
        });
        let addresses = receive
            .await
            .map_err(|_| "DNS lookup stopped unexpectedly.")?
            .map_err(|_| "Hostname resolution failed.")?;
        if addresses.is_empty() {
            Err("Hostname resolved to no addresses.")
        } else {
            Ok(addresses)
        }
    }
}
fn unique(addresses: impl Iterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let mut result = Vec::new();
    for address in addresses {
        if !result.contains(&address) {
            result.push(address);
            if result.len() == 8 {
                break;
            }
        }
    }
    result
}
