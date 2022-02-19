#![allow(missing_docs)]

use thiserror::Error;
pub use anyhow::{Result, bail};

// TODO fix the integration errors

// All systems use these errors, except for GraphQL resolvers, because they have to return a particular kind of error
#[derive(Error, Debug)]
pub enum DianaError {	
    /// An environment variable had an invalid type.
    /// E.g. a port was given as a hex string for some reason.
	#[error("invalid environment variable type for variable '{0}', expected '{1}'")]
    InvalidEnvVarType(String, String),
	
    /// A required part of the GraphQL context was not found.
	#[error("required graphql context element '{0}' not found")]
    GraphQLContextNotFound(String),
	
    /// A Mutex was poisoned (if `.lock()` failed).
	#[error("mutex '{0}' poisoned")]
    MutexPoisoned(String),
	
    /// The subscriptions server failed to publish data it was asked to. This error is usually caused by an authentication failure.
	#[error("failed to publish data to the subscriptions server, this is most likely due to an authentication failure")]
    SubscriptionDataPublishFailed,
	
    /// An invalid indicator string was used when trying to convert a timestring into a datetime.
	#[error("invalid indicator '{0}' in timestring, must be one of: s, m, h, d, w, M, y")]
    InvalidDatetimeIntervalIndicator(String),
	
    /// There was an unauthorised access attempt.
	#[error("unable to comply with request due to lack of valid and sufficient authentication")]
    Unauthorised,
	
	/// One or more required builder fields weren't set up.
	#[error("some required builder fields haven't been instantiated")]
	IncompleteBuilderFields,
	
    /// The creation of an HTTP response for Lambda or its derivatives failed.
	#[error("the builder for an http response (netlify_lambda_http) returned an error")]
    HttpResponseBuilderFailed,
	
    /// There was an attempt to create a subscriptions server without declaring its existence or configuration in the [Options].
	#[error("you tried to create a subscriptions server without configuring it in the options")]
    InvokedSubscriptionsServerWithInvalidOptions,
	
    /// There was an attempt to initialize the GraphiQL playground in a production environment.
	#[error("you tried to initialize the GraphQL playground in production, which is not supported due to authentication issues")]
    AttemptedPlaygroundInProduction,

    /// There was an error in one of the integrations.
	#[error("the following error occurred in the '{0}' integration library: {1}")]
    IntegrationError(String, String),
	
	#[error("unknown IO issue")]
    Io(::std::io::Error),
	
	#[error("unknown EnvVar issue")]
    EnvVar(::std::env::VarError),
	
	#[error("unknown Reqwest issue")]
    Reqwest(::reqwest::Error),
	
	#[error("unknown JSON issue")]
    Json(::serde_json::Error),
	
	#[error("unknown JSON web token issue")]
    JsonWebToken(::jsonwebtoken::errors::Error),
	
    #[error("unknown error")]
    Unknown,
}

/// A wrapper around [`async_graphql::Result<T>`](async_graphql::Result).
/// You should use this as the return type for any of your own schemas that might return errors.
/// # Example
/// ```rust
/// use diana::errors::GQLResult;
///
/// async fn api_version() -> GQLResult<String> {
///     // Your code here
///     Ok("test".to_string())
/// }
/// ```
pub type GQLResult<T> = async_graphql::Result<T>;
/// A wrapper around [`async_graphql::Error`].
/// If any of your schemas need to explicitly create an error that only exists in them (and you're not using something like [mod@error_chain]),
/// you should use this.
/// # Example
/// ```rust
/// use diana::errors::{GQLResult, GQLError};
///
/// async fn api_version() -> GQLResult<String> {
///     let err = GQLError::new("Test error!");
///     // Your code here
///     Err(err)
/// }
/// ```
pub type GQLError = async_graphql::Error;
