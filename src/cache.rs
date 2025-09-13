use bb8::Pool;
use bb8_redis::{
    RedisConnectionManager,
    redis::{Client, ConnectionLike},
};

#[derive(Debug)]
pub struct Storage {
    pub pool: Pool<RedisConnectionManager>,
}

impl Storage {
    pub async fn new(url: &str) -> anyhow::Result<Self> {
        let mut client = Client::open(url)?;

        if client.check_connection() {
            let manager = RedisConnectionManager::new(url)?;
            let pool = Pool::builder().max_size(20).build(manager).await?;

            Ok(Self { pool })
        } else {
            anyhow::bail!("failed to connect to redis");
        }
    }
}
