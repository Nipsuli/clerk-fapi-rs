use clerk_fapi_rs::{models::ClientPeriodUser, Clerk, ClerkFapiConfiguration};
// No need for client_period_client import anymore
use dioxus::{logger::tracing::info, prelude::*};

// Get the Clerk publishable key from environment
pub const CLERK_PUBLISHABLE_KEY: &str = env!("CLERK_PUBLISHABLE_KEY");

/// State for Clerk authentication
#[derive(Clone, Debug, PartialEq)]
pub enum ClerkStatus {
    Loading,
    SignedIn(ClientPeriodUser),
    SignedOut,
    Error(String),
}

/// A Clerk context that allows sharing the clerk client across components
#[derive(Clone)]
pub struct ClerkContext {
    pub client: Clerk,
    pub status: ClerkStatus,
}

/// Create a Clerk provider component that sets up the Clerk context
#[component]
pub fn ClerkProvider(children: Element) -> Element {
    // Create a Clerk client
    let clerk_config =
        ClerkFapiConfiguration::new_browser(CLERK_PUBLISHABLE_KEY.to_string(), None, None)
            .expect("Failed to create Clerk config");

    let clerk_client = Clerk::new(clerk_config);

    // Reactive state for our Clerk context
    let clerk_status = use_signal_sync(|| ClerkStatus::Loading);

    // Create context value
    let context = use_signal(|| ClerkContext {
        client: clerk_client.clone(),
        status: clerk_status.read().clone(),
    });

    // Set up callback to update status when auth state changes
    // Note: Using callbacks with Dioxus signals would require a thread-safe storage
    // which is more complex. For this example, we'll rely on manual refresh.

    // Initialize Clerk client
    {
        let client = clerk_client.clone();

        use_effect(move || {
            to_owned![client, clerk_status];

            spawn(async move {
                // Status is already set to Loading by default
                match client.load().await {
                    Ok(_) => client.add_listener(move |_client, _session, user, _org| {
                        to_owned![clerk_status];
                        info!("Got user {:?}", user);
                        if let Some(user) = user {
                            clerk_status.set(ClerkStatus::SignedIn(user));
                        } else {
                            clerk_status.set(ClerkStatus::SignedOut);
                        }
                    }),
                    Err(e) => {
                        clerk_status.set(ClerkStatus::Error(e.to_string()));
                    }
                }
            });
        });
    }

    // Update context when clerk status changes
    {
        use_effect(move || {
            to_owned![context];
            let status = clerk_status.read().clone();
            context.with_mut(|ctx| {
                ctx.status = status;
            });
        });
    }

    // Provide context
    provide_context(context);

    // Use suspense boundary for clean loading state
    rsx! {
        SuspenseBoundary {
            fallback: move |_ctx| rsx! {
                div {
                    class: "clerk-loading p-4 text-center",
                    "Loading authentication..."
                }
            },
            children
        }
    }
}

/// Custom hook to access the Clerk context
#[inline]
pub fn use_clerk() -> Signal<ClerkContext> {
    // Use context returns the Signal directly, not an Option<Signal>
    use_context::<Signal<ClerkContext>>()
}
