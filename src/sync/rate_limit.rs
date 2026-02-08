use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const BACKOFF_SECONDS: &[u64] = &[60, 120, 240];

/// Check if an asanaclient error is a 429 rate limit.
/// asanaclient's handle_response() converts 429 to `Error::Api { message }`
/// containing "429" in the message, since it discards headers.
pub fn is_429_error(e: &asanaclient::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429") || msg.to_lowercase().contains("rate limit")
}

/// Retry an API call expression with exponential backoff on 429 errors.
///
/// Usage: `retry_api!(client.projects().get_full(gid))`
///
/// The expression is re-evaluated on each retry attempt. This is a macro
/// because async closures that return borrowed futures can't satisfy `Fn`.
macro_rules! retry_api {
    ($expr:expr) => {{
        let mut _attempt: u32 = 0;
        loop {
            match $expr.await {
                Ok(val) => break Ok::<_, crate::error::Error>(val),
                Err(e) => {
                    if $crate::sync::rate_limit::is_429_error(&e) && _attempt < 3 {
                        let wait = [60u64, 120, 240]
                            .get(_attempt as usize)
                            .copied()
                            .unwrap_or(240);
                        log::warn!(
                            "Rate limited (429). Waiting {wait}s before retry {}/3",
                            _attempt + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                        _attempt += 1;
                    } else {
                        break Err(crate::error::Error::Api(e));
                    }
                }
            }
        }
    }};
}

pub(crate) use retry_api;

/// Sleep for the backoff duration (for use in non-macro contexts).
pub async fn backoff_sleep(attempt: u32) {
    let wait = BACKOFF_SECONDS
        .get(attempt as usize)
        .copied()
        .unwrap_or(240);
    log::warn!(
        "Rate limited (429). Waiting {wait}s before retry {}/{}",
        attempt + 1,
        MAX_RETRIES
    );
    tokio::time::sleep(Duration::from_secs(wait)).await;
}
