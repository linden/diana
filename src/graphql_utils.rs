// Utility functions for GraphQL resolvers
use std::sync::{Mutex, MutexGuard};
use tokio_stream::Stream;
use anyhow::{Result, bail};

use crate::auth::auth_state::AuthState;
use crate::pubsub::PubSub;

use crate::errors::DianaError;

/// Checks to see if the given authentication state matches the series of given claims. This must be provided with the authentication state,
/// a series of claims to check against, and code to execute if the user is authenticated. This will call [`bail!`] with an [`DianaError::Unauthorised`](crate::errors::DianaError::Unauthorised)
/// error if the user is unauthenticated, so **that must be handled in your function's return type**!
/// # Example
/// This is a simplified version of the internal logic that publishes data to the subscriptions server.
/// ```
/// use diana::{
///     errors::{Result, GQLResult},
///     graphql_utils::get_auth_data_from_ctx,
///     async_graphql::{Object as GQLObject},
///     if_authed,
/// };
///
/// #[derive(Default, Clone)]
/// pub struct PublishMutation;
/// #[GQLObject]
/// impl PublishMutation {
///     async fn publish(
///         &self,
///         raw_ctx: &async_graphql::Context<'_>,
///         channel: String,
///         data: String,
///     ) -> Result<bool> {
///         let auth_state = get_auth_data_from_ctx(raw_ctx)?;
///         if_authed!(
///             auth_state,
///             {
///                 "role" => "graphql_server"
///             },
///             {
///                 // Your code here
///                 Ok(true)
///             }
///         )
///     }
/// }
///
/// # fn main() {}
/// ```
// TODO mark as deprecated
#[macro_export]
#[deprecated(
    since = "0.2.8",
    note = "please use `is_authed!` instead, it exposes a boolean and lets you use your own error logic"
)]
macro_rules! if_authed(
    ($auth_state:expr, { $($key:expr => $value:expr),+ }, $code:block) => {
        {
            // Create a HashMap out of the given test claims
            let mut test_claims: ::std::collections::HashMap<&str, &str> = ::std::collections::HashMap::new();
            $(
                test_claims.insert($key, $value);
            )+
            // Match the authentication state with those claims now
            if $auth_state.has_claims(test_claims) {
                $code
            } else {
                Err($crate::errors::DianaError::Unauthorised.into())
            }
        }
     };
);

/// Checks to see if the given authentication state matches the series of given claims. This must be provided with the authentication state,
/// a series of claims to check against. It will then return a boolean as to whether or not the user is authorized.
/// This should be used instead of [`if_authed!`].
/// # Example
/// This is a simplified version of the internal logic that publishes data to the subscriptions server.
/// ```
/// use diana::{
///     errors::{Result, GQLResult, bail, DianaError},
///     graphql_utils::get_auth_data_from_ctx,
///     async_graphql::{Object as GQLObject},
///     is_authed,
/// };
///
/// #[derive(Default, Clone)]
/// pub struct PublishMutation;
/// #[GQLObject]
/// impl PublishMutation {
///     async fn publish(
///         &self,
///         raw_ctx: &async_graphql::Context<'_>,
///         channel: String,
///         data: String,
///     ) -> Result<bool> {
///         if is_authed!(
///             get_auth_data_from_ctx(raw_ctx)?,
///             {
///                 "role" => "graphql_server"
///             }
///         ) {
///             // Your code here
///             Ok(true)
///         } else {
///             // Your error handling code here
///             bail!(DianaError::Unauthorised)
///         }
///     }
/// }
///
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! is_authed(
    ($auth_state:expr, { $($key:expr => $value:expr),+ }) => {
        {
            // Create a HashMap out of the given test claims
            let mut test_claims: ::std::collections::HashMap<&str, &str> = ::std::collections::HashMap::new();
            $(
                test_claims.insert($key, $value);
            )+
            // Match the authentication state with those claims now
            $auth_state.has_claims(test_claims)
        }
     };
);

/// Gets a subscription stream to events published on a particular channel from the context of a GraphQL resolver.
/// **This must only be used in subscriptions! It will not work anywhere else!**
/// This returns a pre-created stream which you should manipulate if necessary.
/// All data sent via the publisher from the queries/mutations system will land here **in string format**. Serialization is up to you.
/// # Example
/// ```
/// use diana::{
///     stream,
///     graphql_utils::get_stream_for_channel_from_ctx,
///     errors::GQLResult,
///     async_graphql::{Subscription as GQLSubscription, SimpleObject as GQLSimpleObject},
/// };
/// use tokio_stream::{Stream, StreamExt};
/// use serde::Deserialize;
///
/// #[derive(Deserialize, GQLSimpleObject)]
/// struct User {
///     username: String
/// }
///
/// #[derive(Default, Clone)]
/// pub struct Subscription;
/// #[GQLSubscription]
/// impl Subscription {
///     async fn new_users(
///         &self,
///         raw_ctx: &async_graphql::Context<'_>,
///     ) -> impl Stream<Item = GQLResult<User>> {
///         // Get a direct stream from the context on a certain channel
///         let stream_result = get_stream_for_channel_from_ctx("new_user", raw_ctx);
///
///         // We can manipulate the stream using the stream macro from async-stream
///         stream! {
///             let stream = stream_result?;
///             for await message in stream {
///                 // Serialise the data as a user
///                 let new_user: User = serde_json::from_str(&message).map_err(|_err| "couldn't serialize given data correctly".to_string())?;
///                 yield Ok(new_user);
///             }
///         }
///     }
/// }
/// # fn main() {}
/// ```
///
pub fn get_stream_for_channel_from_ctx(
    channel: &str,
    raw_ctx: &async_graphql::Context<'_>,
) -> Result<impl Stream<Item = String>> {
    // Get the PubSub mutably
    let mut pubsub = get_pubsub_from_ctx(raw_ctx)?;
    // Return a stream on the given channel
    Ok(pubsub.subscribe(channel))
}

/// Gets authentication data from the context of a GraphQL resolver.
/// This should only fail if the server is constructed without authentication middleware (which shouldn't be possible with the exposed API
/// surface of this crate).
pub fn get_auth_data_from_ctx<'a>(
    raw_ctx: &'a async_graphql::Context<'_>,
) -> Result<&'a AuthState> {
    let auth_state = raw_ctx
        .data::<AuthState>()
        .map_err(|_err| DianaError::GraphQLContextNotFound("auth_state".to_string()))?;

    Ok(auth_state)
}
/// Gets the internal PubSub from the context of a GraphQL resolver. You should never need to use this.
#[doc(hidden)]
pub fn get_pubsub_from_ctx<'a>(
    raw_ctx: &'a async_graphql::Context<'_>,
) -> Result<MutexGuard<'a, PubSub>> {
    // We store the PubSub instance as a Mutex because we need it sent/synced between threads as a mutable
    let pubsub_mutex = raw_ctx
        .data::<Mutex<PubSub>>()
        .map_err(|_err| DianaError::GraphQLContextNotFound("pubsub".to_string()))?;
    let pubsub = pubsub_mutex
        .lock()
        .map_err(|_err| DianaError::MutexPoisoned("pubsub".to_string()))?;

    Ok(pubsub)
}
