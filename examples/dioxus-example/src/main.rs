use crate::use_clerk::*;
use dioxus::prelude::*;

mod use_clerk;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    // Initialize logging for the error messages
    wasm_logger::init(wasm_logger::Config::default());
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        // Wrap the entire app with the Clerk provider
        ClerkProvider {
            Router::<Route> {}
        }
    }
}

/// Home page component
#[component]
fn Home() -> Element {
    // Use our improved clerk custom hooks
    let clerk_ctx = use_clerk();

    rsx! {
        div { class: "container mx-auto p-4",
            div {
                id: "clerk-status",
                class: "bg-white shadow-md rounded p-4 mb-4",
                h2 { class: "text-xl text-black font-semibold mb-2", "Clerk Status" }

                div { class: "mt-2",
                    // Display different content based on auth status
                    match clerk_ctx.read().status {
                        ClerkStatus::Loading => {
                            rsx! {
                                div { class: "p-2 bg-blue-100 text-blue-800 rounded",
                                    "Loading authentication status..."
                                }
                            }
                        },
                        ClerkStatus::SignedIn(ref user) => {
                            let display_name = match (&user.first_name, &user.last_name) {
                                (Some(first), Some(last)) => format!("{} {}", first, last),
                                (Some(first), None) => first.clone(),
                                (None, Some(last)) => last.clone(),
                                (None, None) => "User".to_string()
                            };

                            let email_display = match &user.primary_email_address_id {
                                Some(email_id) => format!("Email: {}", user.email_addresses
                                    .iter()
                                    .find(|e| &e.id == email_id)
                                    .map_or(email_id, |e| &e.email_address)
                                ),
                                None => "No email provided".to_string()
                            };

                            let user_id = format!("User ID: {}", user.id);

                            rsx! {
                                div { class: "p-2 bg-green-100 text-green-800 rounded",
                                    div {
                                        "Signed in as: ",
                                        strong { "{display_name}" }
                                    }
                                    p { class: "mt-2", "{email_display}" }
                                    p { class: "text-xs text-gray-500 mt-1", "{user_id}" }
                                }
                            }
                        },
                        ClerkStatus::SignedOut => {
                            rsx! {
                                div { class: "p-2 bg-yellow-100 text-yellow-800 rounded mb-4",
                                    "You are not signed in"
                                }

                                // Show the sign-in form
                                SignIn {}
                            }
                        },
                        ClerkStatus::Error(ref err) => {
                            rsx! {
                                div { class: "p-2 bg-red-100 text-red-800 rounded",
                                    "Error: {err}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        header { class: "bg-gray-800 text-white p-4",
            div { class: "container mx-auto flex justify-between items-center",
                div { class: "text-xl font-bold", "Clerk Dioxus Example" }

                nav {
                    ul { class: "flex space-x-4 items-center",
                        li {
                            Link {
                                class: "hover:text-blue-300",
                                to: Route::Home {},
                                "Home"
                            }
                        }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}

/// SignIn component for email authentication
#[component]
fn SignIn() -> Element {
    let clerk_ctx = use_clerk();

    let mut email = use_signal(String::new);
    let mut code = use_signal(String::new);
    let mut status = use_signal(|| None::<String>);
    let sign_in_id = use_signal(|| None::<String>);

    // Track sign-in flow step
    // 0: Initial email input
    // 1: Code verification
    let step = use_signal(|| 0);

    let on_send_email_code = move |_| {
        // Basic email validation
        let email_value = email.read().trim().to_string();
        if email_value.is_empty() {
            status.set(Some("Please enter an email address".to_string()));
            return;
        }

        if !email_value.contains('@') {
            status.set(Some("Please enter a valid email address".to_string()));
            return;
        }

        status.set(Some("Sending email code...".to_string()));

        let email_value = email_value.clone();
        to_owned![status, step, sign_in_id, clerk_ctx];

        spawn(async move {
            if let Ok(sign_in_response) = clerk_ctx
                .read()
                .client
                .get_fapi_client()
                .create_sign_in(
                    None,               // origin
                    Some("email_code"), // strategy
                    Some(&email_value), // identifier
                    None,               // password
                    None,               // ticket
                    None,               // redirect_url
                    None,               // action_complete_redirect_url
                    None,               // transfer
                    None,               // code
                    None,               // token
                    None,               // oidc_login_hint
                    None,               // oidc_prompt
                )
                .await
            {
                sign_in_id.set(Some(sign_in_response.id));

                status.set(Some(
                    "Code sent! Check your email and enter the code below.".to_string(),
                ));
                step.set(1);
            }
        });
    };

    let on_verify_code = move |_| {
        let code_value = code.read().trim().to_string();
        if code_value.is_empty() {
            status.set(Some("Please enter the verification code".to_string()));
            return;
        }

        if code_value.len() != 6 {
            status.set(Some("Please enter a valid verification code".to_string()));
            return;
        }

        status.set(Some("Verifying code...".to_string()));

        let code_value = code_value.clone();
        to_owned![status, clerk_ctx, sign_in_id];

        spawn(async move {
            if let Some(sign_in_id) = sign_in_id.read().as_ref() {
                if let Ok(_verify_response) = clerk_ctx
                    .read()
                    .client
                    .get_fapi_client()
                    .attempt_sign_in_factor_one(
                        sign_in_id,
                        "email_code",      // strategy
                        None,              // origin
                        Some(&code_value), // code
                        None,              // password
                        None,              // signature
                        None,              // token
                        None,              // ticket
                        None,              // public_key_credential
                    )
                    .await
                {
                    // Update UI state
                    status.set(Some(
                        "Verification successful! Signing you in...".to_string(),
                    ));
                } else {
                    status.set(Some("Failed to verify code. Please try again.".to_string()));
                }
            } else {
                status.set(Some("No sign-in in progress. Please restart.".to_string()));
            }
        });
    };

    // Handle resend code button click
    let on_resend_code = move |_| {
        status.set(Some("Resending code...".to_string()));

        // Actually resend the code
        to_owned![status, sign_in_id, clerk_ctx];
        spawn(async move {
            if let Some(sign_in_id) = sign_in_id.read().as_ref() {
                // Resend the verification code
                if (clerk_ctx
                    .read()
                    .client
                    .get_fapi_client()
                    .prepare_sign_in_factor_one(
                        sign_in_id,
                        "email_code", // strategy
                        None,         // origin
                        None,         // email_address_id
                        None,         // phone_number_id
                        None,         // web3_wallet_id
                        None,         // passkey_id
                        None,         // redirect_url
                        None,         // action_complete_redirect_url
                        None,         // oidc_login_hint
                        None,         // oidc_prompt
                    )
                    .await)
                    .is_ok()
                {
                    // Update UI state
                    status.set(Some("New code sent! Check your email.".to_string()));
                } else {
                    status.set(Some("Failed to resend code. Please try again.".to_string()));
                }
            } else {
                status.set(Some("No sign-in in progress. Please restart.".to_string()));
            }
        });
    };

    rsx! {
        div { class: "bg-white shadow-md rounded p-4",
            h2 { class: "text-xl font-semibold mb-4", "Sign In with Email" }

            div {
                class: "space-y-4",

                // Email input field (always shown)
                div { class: "flex flex-col",
                    label { class: "mb-1 text-sm font-medium text-gray-700", r#for: "email", "Email Address" }
                    input {
                        id: "email",
                        r#type: "email",
                        placeholder: "Enter your email",
                        value: "{email}",
                        oninput: move |evt| email.set(evt.value().clone()),
                        class: "p-2 border border-gray-300 rounded text-black focus:outline-none focus:ring-2 focus:ring-blue-400",
                        disabled: *step.read() == 1,
                    }
                }

                // Show status message if any
                {
                    match status.read().as_ref() {
                        Some(message) => {
                            let status_class = if message.starts_with("Please") {
                                "p-2 bg-red-100 text-red-800 rounded text-sm my-2"
                            } else if message.contains("successful") {
                                "p-2 bg-green-100 text-green-800 rounded text-sm my-2"
                            } else {
                                "p-2 bg-blue-100 text-blue-800 rounded text-sm my-2"
                            };

                            rsx! {
                                div { class: status_class, "{message}" }
                            }
                        },
                        None => rsx!{}
                    }
                }

                // Show the appropriate button or input based on the current step
                {
                    match *step.read() {
                        0 => rsx! {
                            // Initial step - show send code button
                            button {
                                onclick: on_send_email_code,
                                class: "w-full bg-blue-500 hover:bg-blue-600 text-white font-medium py-2 px-4 rounded focus:outline-none focus:ring-2 focus:ring-blue-400",
                                "Send Email Code"
                            }
                        },
                        1 => rsx! {
                            // Code verification step
                            div { class: "mt-4 flex flex-col",
                                label { class: "mb-1 text-sm font-medium text-gray-700", r#for: "code", "Verification Code" }
                                input {
                                    id: "code",
                                    r#type: "text",
                                    placeholder: "Enter verification code",
                                    value: "{code}",
                                    oninput: move |evt| code.set(evt.value().clone()),
                                    class: "p-2 border border-gray-300 rounded text-black focus:outline-none focus:ring-2 focus:ring-blue-400",
                                }

                                div { class: "flex flex-col space-y-2 mt-4 sm:flex-row sm:space-y-0 sm:space-x-2",
                                    button {
                                        onclick: on_verify_code,
                                        class: "flex-1 bg-blue-500 hover:bg-blue-600 text-white font-medium py-2 px-4 rounded focus:outline-none focus:ring-2 focus:ring-blue-400",
                                        "Verify Code"
                                    }

                                    button {
                                        onclick: on_resend_code,
                                        class: "flex-1 bg-gray-200 hover:bg-gray-300 text-gray-800 font-medium py-2 px-4 rounded focus:outline-none focus:ring-2 focus:ring-gray-400",
                                        "Resend Code"
                                    }
                                }
                            }
                        },
                        _ => rsx!{} // Shouldn't happen but handle gracefully
                    }
                }
            }
        }
    }
}
