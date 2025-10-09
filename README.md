# Rust Clerk REST Frontend API

An unofficial Rust SDK for the Clerk REST Frontend API.

## Status

Work in progress. Works and is used in production. But as [reconfigured](https://reconfigured.io)
is using only parts of the API there might be things that are broken in other APIs.

## Core idea

This crate is quite thin wrapper on top of the REST Frontend API.
`Clerk` is a statefull client exposing the full Clerk FAPI methods via
`Clerk::get_fapi_client`. It does not implement similar objects with
methods as example the Javscript Clerk does, only the direct FAPI
endpoints. But it does keep the current state of the client in sync
allowing one signin and call api methods as signed in user.

The `src/apis` and `src/models` are generated based on the `fapi_swagger.json`.
There seems to be small issues in the clerk API spec and it does not reflect the
reality in all of the cases. Those cases where I've run into are fixed by hand.

By default the state is stored in in `HashMap` but if one wants to
add some persistent state one can provide anything that implments
the `clerk_fapi_rs::configuration::Store` trait.

The main usage of the Clerk FAPI happens via the `Clerk::get_fapi_client`
method. The returned `ClerkFapiClient` supports fully typed [Clerk FAPI](https://clerk.com/docs/reference/frontend-api).

Clerk communicates the state of sessions back by piggybagging the requests
and returns full `ClientPeriodClient` as part of most api calls. The
`Clerk` updates it's own state based on the responses and triggers
optional listeners which one can register with `clerk::add_listener`

The type of lister:

```rs
pub type Listener =
    Arc<dyn Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync>;
```

There are only few convenience methods provided directly on the `Clerk`:

- `get_token` to get session token that can be used to authenticate backend calls
- `sign_out` to, well, sign out
- `set_active` to activate session or organization in session

And to read current state there are helper acccess methods:

- `Clerk::environment()` for the current Clerk instance configs
- `Clerk::client()` to access full `ClientPeriodClient`
- `Clerk::session()` to access currently active session parsed from `ClientPeriodClient`
- `Clerk::user()` to access current user parsed from `ClientPeriodClient`
- `Clerk::organization()` to access current organization parsed from `ClientPeriodClient`

## Basic Usage

```rust
use clerk_fapi_rs::{clerk::Clerk, configuration::ClerkFapiConfiguration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let public_key = todo!("Load the way you want");

    // Init configuration
    let config = ClerkFapiConfiguration::new(
        public_key, // String
        None,       // proxy
        None,       // domain
    )?;

    // Or in browser
    let config = ClerkFapiConfiguration::new_browser(
        public_key, // String
        None,       // proxy
        None,       // domain
    )?;

    // Or with store
    let config = ClerkFapiConfiguration::new_with_store(
        public_key, // String
        None,       // proxy
        None,       // domain
        Some(Arc::new(my_clerk_store)),
        None,       // store_prefix
        ClientKind::NonBrowser,
    )?;

    // Initialize Clerk client
    let clerk = Clerk::new(config);

    // Load client, it loads the Environment and Client from API
    clerk.load().await?;

    // If one uses persisted store and want to use cached values
    clerk.load(true).await?;

    // Get fapi client
    let fapi = clerk.get_fapi_client();

    // ... do calls with fapi
}
```

## Contributing

PR are welcome.

## Release

With [cargo-release](https://crates.io/crates/cargo-release)
