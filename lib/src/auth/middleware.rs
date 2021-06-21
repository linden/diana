use std::pin::Pin;
use std::task::{Context, Poll};
use std::result::Result as StdResult;
use actix_web::{
    dev::{
        Service, Transform,
        ServiceRequest, ServiceResponse
    },
    Error,
    HttpResponse,
    HttpMessage
};
use futures::{
    future::{ok, Ready},
    Future
};

use crate::errors::*;
use crate::auth::jwt::{get_jwt_secret, validate_and_decode_jwt};
use crate::auth::auth_state::{AuthState, AuthToken};

// Extracts an authentication state from the token
fn get_token_state_from_req(req: &ServiceRequest) -> Result<AuthState> {
    // Get the authorisation header from the request
    let raw_auth_header = req
                            .headers()
                            .get("AUTHORIZATION");
    // Get the bearer token from that if it exists
    // This will end up as an option
    let bearer_token = match raw_auth_header {
        Some(header) => {
            let header_str = header.to_str();
            let header_str = match header_str {
                Ok(header_str) => {
                    let bearer_token = header_str.split("Bearer")
                                .collect::<Vec<&str>>()
                                .get(1) // Get everything apart from that first element
                                .map(|token| token.trim());
                    bearer_token
                },
                Err(_) => None
            };
            header_str
        },
        None => None
    };

    // Decode the bearer token into an authentication state
    match bearer_token {
        Some(token) => {
            let jwt_secret = get_jwt_secret(None)?; // We'll use the environment variable
            let decoded_jwt = validate_and_decode_jwt(&token, &jwt_secret);

            match decoded_jwt {
                Some(claims) => Ok(AuthState::Authorised(
                    AuthToken(claims)
                )),
                None => Ok(AuthState::InvalidToken) // The token is invalid
            }
        }
        None => Ok(AuthState::NoToken) // No token exists
    }
}

// The block state chosen may have unforseen security implications, please choose wisely!
#[derive(Debug, Clone, Copy)]
enum AuthCheckBlockState {
    AllowAll, // Allows anything through, adding the auth parameters to the request for later processing
    BlockUnauthenticated, // Blocks missing/invalid tokens (all requests must be authenticated)
    AllowMissing // Only block if an invalid token is given (if no token, allowed)
}

// Create a factory for authentication middleware
pub struct AuthCheck {
    block_state: AuthCheckBlockState // This defines whether or not we should block requests without a token or with an invalid one
}
impl AuthCheck {
    // These functions allow us to initialise the middleware factory (and thus the middleware itself) with custom options
    pub fn block_unauthenticated() -> Self {
        Self {
            block_state: AuthCheckBlockState::BlockUnauthenticated, // We block by default
        }
    }
    pub fn allow_missing() -> Self {
        Self {
            block_state: AuthCheckBlockState::AllowMissing,
        }
    }
    pub fn allow_all() -> Self {
        Self {
            block_state: AuthCheckBlockState::AllowAll
        }
    }
}

// This is what we'll actually call, all it does is create the middleware and define all its properties
impl<S> Transform<S> for AuthCheck
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    // All the properties of the middleware need to be defined here
    // We could do this with `wrap_fn` instead, but this approach gives far greater control
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = AuthCheckMiddleware<S>;
    type Future = Ready<StdResult<Self::Transform, Self::InitError>>;

    // This will be called internally by Actix Web to create our middleware
    // All this really does is pass the service itself (handler basically) over to our middleware
    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthCheckMiddleware { service, block_state: self.block_state })
    }
}

// The actual middleware
pub struct AuthCheckMiddleware<S> {
    service: S,
    block_state: AuthCheckBlockState // This will be passed in from whatever is set for the factory
}

impl<S> Service for AuthCheckMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    // More properties for Actix Web
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = StdResult<Self::Response, Self::Error>>>>;

    // Stock function for asynchronous operations
    // The context here has nothing to do with our app's internal context whatsoever!
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        // Check the token
        let token_state = get_token_state_from_req(&req);
        match token_state {
            // We hold `token_state` as the AuthState variant so we don't pointlessly insert a Result into the request extensions
            Ok(token_state @ AuthState::Authorised(_)) => {
                // The token is authorised, these will always be let through
                req.extensions_mut().insert(token_state);
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                })
            },
            Ok(token_state @ AuthState::InvalidToken) => {
                if let AuthCheckBlockState::AllowAll = self.block_state {
                    // Anything is being let through, pass the state to the handler
                    req.extensions_mut().insert(token_state);
                    let fut = self.service.call(req);
                    Box::pin(async move {
                        let res = fut.await?;
                        Ok(res)
                    })
                } else {
                    // We're blocking unauthenticated requests, return a 403 error
                    Box::pin(async move {
                        Ok(ServiceResponse::new(
                            req.into_parts().0, // Eliminates the payload of the request
                            HttpResponse::Unauthorized().finish() // In the playground this will come up as bad JSON, it's a direct HTTP response
                        ))
                    })
                }
            },
            Ok(token_state @ AuthState::NoToken) => {
                if let AuthCheckBlockState::AllowAll | AuthCheckBlockState::AllowMissing = self.block_state {
                    // Missing tokens are being let through, pass the state to the handler
                    req.extensions_mut().insert(token_state);
                    let fut = self.service.call(req);
                    Box::pin(async move {
                        let res = fut.await?;
                        Ok(res)
                    })
                } else {
                    // We're blocking unauthenticated requests, return a 403 error
                    Box::pin(async move {
                        Ok(ServiceResponse::new(
                            req.into_parts().0, // Eliminates the payload of the request
                            HttpResponse::Unauthorized().finish() // In the playground this will come up as bad JSON, it's a direct HTTP response
                        ))
                    })
                }
            },
            Err(_) => {
                // Middleware failed, we shouldn't let this proceed to the request just in case
                // This error could be triggered by a failure in transforming the token from base64, meaning the error can be caused forcefully by an attacker
                // In that scenario, we can't allow the bypassing of this layer
                Box::pin(async move {
                    Ok(ServiceResponse::new(
                        req.into_parts().0, // Eliminates the payload of the request
                        HttpResponse::InternalServerError().finish()
                    ))
                })
            }
        }
    }
}
