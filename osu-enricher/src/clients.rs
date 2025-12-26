use rosu_v2::Osu;
use governor::RateLimiter;
use governor::state::InMemoryState;
use governor::state::direct::NotKeyed;
use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use anyhow::Result;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct OsuClient {
    pub client: Osu,
    pub rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>,
}

pub struct OsuClientPool {
    clients: Vec<OsuClient>,
    current: AtomicUsize,
}

impl OsuClientPool {
    pub async fn new(credentials: Vec<(u64, String)>) -> Result<Self> {
        let mut clients = Vec::new();
        for (client_id, client_secret) in credentials {
            let client = Osu::new(client_id, client_secret).await?;
            let rate_limiter = RateLimiter::direct(governor::Quota::per_minute(std::num::NonZeroU32::new(600).unwrap()));
            clients.push(OsuClient {
                client,
                rate_limiter,
            });
        }
        
        Ok(Self {
            clients,
            current: AtomicUsize::new(0),
        })
    }

    pub fn get_next(&self) -> &OsuClient {
        let idx = self.current.fetch_add(1, Ordering::SeqCst) % self.clients.len();
        &self.clients[idx]
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}
