# Rust Clerk REST Frontend API

An unofficial Rust SDK for the Clerk REST Frontend API.

## Status

Work in progress. Works and is used in production. But as [reconfigured](https://reconfigured.io)
is using only parts of the API there might be things that are broken in other APIs.

Does expose all the FAPI endpoints, and keeps the local client in sync when API
calls are made. By default in in memory hashmap, but one can pass in own storage
implementation if one requires persistence.

## Basic Usage

Init client
```rust
use clerk_fapi_rs::{clerk::Clerk, configuration::ClerkFapiConfiguration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let public_key = todo!("Load the way you want");

    // Init configuration
    let config = ClerkFapiConfiguration::new(
        public_key, // String
        None, // No proxy
        None, // No special domain
    )?;

    // Initialize Clerk client
    let clerk = Clerk::new(config);

    // Load the client (this fetches client and environment from Clerk API)
    // In case of there is cached client in storage will use cached client
    // and trigger backround refresh of client and environment.
    let clerk = clerk.load().await?;
    
    //...
}
```

Login with email code

```rust

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let clerk = todo!("Load the way you want");
    // ...
    let email = "nipsuli@reconfigured.io";

    // Create sign-in attempt, sends email with code to user
    let sign_in_response = clerk
        .get_fapi_client()
        .create_sign_in(
            Some("email_code"),
            Some(&email), //
            None, // password
            None, // ticket
            None, // redirect_url
            None, // action_complete_redirect_url
            None, // transfer
            None, // code
            None, // token
        )
        .await?;

    let sign_in_id = sign_in_response.response.id;

    let code = todo!("Get the code from the email");

    // Attempt first factor verification
    let verification_response = clerk
        .get_fapi_client()
        .attempt_sign_in_factor_one(
            &sign_in_id,
            Some("email_code"),
            Some(&code),
            None, // password
            None, // signature
            None, // redirect_url - does not make sense here
            None, // action_complete_redirect_url - does not make sense here
            None, // ticket
        )
        .await?;

    if verification_response.response.status
        == clerk_fapi_rs::models::client_period_sign_in::Status::Complete
    {
        println!("Sign in successful!");
    } else {
        println!(
            "Sign in failed. Status: {:?}",
            verification_response.response.status
        );
    }
    // ...
}
```

Signing out
```rust
#[tokio::main]  
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let clerk = todo!("Load the way you want");
    // ...
    clerk.sign_out(None).await?; // In multi session environment can sign out specific session
}
```

Setting active Organization
```rust
#[tokio::main]  
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let clerk = todo!("Load the way you want");
    // ...
    let session_id = None;
    let organization_id_or_slug = Some("my-mega-organization");

    clerk
        .set_active(
            session_id, // Option<String>
            organization_id_or_slug // Option<String>
        )
        .await?;
}
```

Listening to auth changes

```rust
#[tokio::main]  
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let clerk = todo!("Load the way you want");
    // ...
    // Add a listener that will be called whenever the client state changes
    clerk.add_listener(|client, session, user, organization| {
        println!("Client: {:?}", client);
        println!("Session: {:?}", session);
        println!("User: {:?}", user);
        println!("Organization: {:?}", organization);
    });
}
```

There are some other helper methods such as:
```rust
use clerk_fapi_rs::models::{
    ClientPeriodClient as Client, ClientPeriodEnvironment as Environment,
    ClientPeriodOrganization as Organization, ClientPeriodSession as Session,
    ClientPeriodUser as User,
};

#[tokio::main]  
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let clerk = todo!("Load the way you want");
    // ...
    // Check if the client is loaded
    let is_loaded: bool = clerk.loaded();
    
    // Get the current environment information
    let env: Option<Environment> = clerk.environment();
    
    // Get the current client data 
    let client: Option<Client> = clerk.client();
    
    // Get the current session
    let session: Option<Session> = clerk.session();
    
    // Get the authenticated user
    let user: Option<User> = clerk.user();
    
    // Get the active organization
    let org: Option<Organization> = clerk.organization();
    
    // Get a session JWT token
    let token: Option<String> = clerk.get_token(None, None).await?;
}
```

And the full [Clerk FAPI](https://clerk.com/docs/reference/frontend-api)
is available as fully typed methods via the `clerk.get_fapi_client()`.

## Contributing

PR are welcome.

## Release

With [cargo-release](https://crates.io/crates/cargo-release)
