use clerk_fapi_rs::{models::*, ClerkFapiClient, ClerkFapiConfiguration};
// No need for client_period_client import anymore
use dioxus::prelude::*;

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
    pub client: ClerkFapiClient,
    pub status: ClerkStatus,
}

/// Create a Clerk provider component that sets up the Clerk context
#[component]
pub fn ClerkProvider(children: Element) -> Element {
    // Create a Clerk client
    let clerk_config = ClerkFapiConfiguration::new(
        CLERK_PUBLISHABLE_KEY.to_string(),
        None,
        None,
    ).expect("Failed to create Clerk config");
    
    let clerk_client = ClerkFapiClient::new(clerk_config).expect("Failed to create Clerk client");
    
    // Reactive state for our Clerk context
    let clerk_status = use_signal(|| ClerkStatus::Loading);
    
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
        let clerk_status = clerk_status.clone();
        let context = context.clone();
        
        use_effect(move || {
            to_owned![client, clerk_status, context];
            
            spawn(async move {
                match client.get_client().await {
                    Ok(client_response) => {
                        if let Some(client) = client_response.response {
                            // Get session and user
                            if let Some(session_id) = client.last_active_session_id {
                                // Find the session with this ID
                                let active_session = client.sessions.iter()
                                    .find(|s| s.id == session_id);
                                
                                if let Some(session) = active_session {
                                    if let Some(user) = &session.user {
                                        clerk_status.set(ClerkStatus::SignedIn((**user).clone()));
                                    } else {
                                        clerk_status.set(ClerkStatus::SignedOut);
                                    }
                                } else {
                                    clerk_status.set(ClerkStatus::SignedOut);
                                }
                            } else {
                                clerk_status.set(ClerkStatus::SignedOut);
                            }
                        } else {
                            clerk_status.set(ClerkStatus::SignedOut);
                        }
                        
                        // Update the context
                        context.with_mut(|ctx| {
                            ctx.status = clerk_status.read().clone();
                        });
                    }
                    Err(err) => {
                        let error_msg = format!("Error initializing Clerk: {:?}", err);
                        clerk_status.set(ClerkStatus::Error(error_msg.clone()));
                        
                        // Update the context
                        context.with_mut(|ctx| {
                            ctx.status = ClerkStatus::Error(error_msg);
                        });
                    }
                }
            });
        });
    }
    
    // Update context when clerk status changes
    {
        let mut context_clone = context.clone();
        let clerk_status_clone = clerk_status.clone();
        use_effect(move || {
            let status = clerk_status_clone.read().clone();
            context_clone.with_mut(|ctx| {
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

/// Helper hook that returns whether the user is currently signed in
#[inline]
pub fn use_is_signed_in() -> bool {
    matches!(use_clerk().read().status, ClerkStatus::SignedIn(_))
}

/// Helper hook that returns the current user if signed in
#[inline]
pub fn use_user() -> Option<ClientPeriodUser> {
    match &use_clerk().read().status {
        ClerkStatus::SignedIn(user) => Some(user.clone()),
        _ => None,
    }
}

/// The state for sign-in
#[inline]
pub fn use_sign_in_state() -> (Signal<String>, Signal<String>, Signal<Option<Result<(), String>>>) {
    let email = use_signal(|| String::new());
    let password = use_signal(|| String::new());
    let result = use_signal(|| None);
    
    (email, password, result)
}

/// Sign-in handler
pub async fn sign_in(
    client: &ClerkFapiClient, 
    email: &str, 
    password: &str
) -> Result<(), String> {
    if email.is_empty() || password.is_empty() {
        return Err("Email and password are required".to_string());
    }
    
    // Create a sign-in attempt
    client.create_sign_in(
        None,            // origin
        Some("password"), // strategy
        Some(email),     // identifier
        Some(password),  // password
        None, None, None, None, None, None, None, None
    ).await
    .map(|_| ())
    .map_err(|e| format!("Sign-in failed: {:?}", e))
}

/// The state for sign-out
#[inline]
pub fn use_sign_out_state() -> Signal<Option<Result<(), String>>> {
    use_signal(|| None)
}

/// Sign-out handler
pub async fn sign_out(client: &ClerkFapiClient) -> Result<(), String> {
    // Get the active session
    let sessions = client.get_sessions(None)
        .await
        .map_err(|e| format!("Failed to get sessions: {:?}", e))?;
    
    // End the first active session if any exists
    if let Some(session) = sessions.first() {
        client.revoke_session(&session.id, None)
            .await
            .map_err(|e| format!("Failed to revoke session: {:?}", e))?;
    }
    
    Ok(())
}