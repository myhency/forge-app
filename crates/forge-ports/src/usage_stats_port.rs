use async_trait::async_trait;
use forge_domain::UsageStats;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum UsageStatsError {
    #[error("failed to read usage: {0}")]
    ReadFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait UsageStatsPort: Send + Sync {
    /// What every session did, by day and hour in the time zone `utc_offset_seconds` east of UTC.
    /// A first call may read every transcript; later calls read only what changed.
    async fn load(&self, utc_offset_seconds: i32) -> Result<UsageStats, UsageStatsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_usage_stats_port_returns_what_it_is_told() {
        let mut mock = MockUsageStatsPort::new();
        mock.expect_load().withf(|offset| *offset == 32_400).times(1).returning(|_| Ok(UsageStats::default()));
        assert!(mock.load(32_400).await.unwrap().is_empty());
    }
}
