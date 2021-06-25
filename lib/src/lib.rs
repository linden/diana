#![forbid(unsafe_code)]
// This crate has a library architecture for easier testing and code reuse, with binaries that make use of the library in the server crates

pub mod errors;
pub mod graphql_utils; // Users need to be able to access these of course in their schemas
mod graphql;
mod pubsub;
mod options;
mod graphql_server;
mod subscriptions_server;
mod serverless;
mod aws_serverless;
mod auth;
mod routes;

// Public exports accessible from the root (everything the user will need)
pub use crate::graphql_server::create_graphql_server;
pub use crate::subscriptions_server::create_subscriptions_server;
pub use crate::serverless::{ServerlessResponse, run_serverless_req};
pub use crate::aws_serverless::{AwsError, run_aws_req};
pub use crate::options::{Options, OptionsBuilder, AuthCheckBlockState};
pub use crate::pubsub::Publisher;

// Users shouldn't have to install Actix Web themselves for basic usage
pub use actix_web::{App, HttpServer};
// Users also shouldn't have to install the Netlify stuff themselves for basic usage
pub use netlify_lambda_http::{
    handler as create_handler,
    lambda::{Context as LambdaCtx, run as run_lambda},
    IntoResponse as IntoLambdaResponse, Request as LambdaRequest
};
