use dioxus::prelude::*;
use crate::use_clerk::*;

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
const HEADER_SVG: Asset = asset!("/assets/header.svg");
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
    let _is_signed_in = use_is_signed_in();
    let user = use_user();
    
    // For sign-in functionality
    let (mut email, mut password, mut sign_in_result) = use_sign_in_state();
    let mut sign_out_result = use_sign_out_state();
    
    // Handle sign-in form submission
    let handle_sign_in = move |e: FormEvent| {
        e.prevent_default();
        
        let client = clerk_ctx.read().client.clone();
        let email_value = email.read().clone();
        let password_value = password.read().clone();
        
        spawn(async move {
            let result = sign_in(&client, &email_value, &password_value).await;
            sign_in_result.set(Some(result));
        });
    };
    
    // Handle sign-out button click
    let handle_sign_out = move |_| {
        let client = clerk_ctx.read().client.clone();
        
        spawn(async move {
            let result = sign_out(&client).await;
            sign_out_result.set(Some(result));
        });
    };
    
    rsx! {
        div { class: "container mx-auto p-4",
            div { 
                id: "clerk-status",
                class: "bg-white shadow-md rounded p-4 mb-4",
                h2 { class: "text-xl font-semibold mb-2", "Clerk Status" }
                
                div {
                    match &clerk_ctx.read().status {
                        ClerkStatus::Loading => rsx! {
                            div { class: "text-gray-600 p-4 text-center", "Initializing Clerk..." }
                        },
                        ClerkStatus::Error(err) => rsx! {
                            div { class: "bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded", 
                                "Error: {err}" 
                            }
                        },
                        ClerkStatus::SignedIn(_) => rsx! {
                            div { class: "bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded mb-4",
                                "You are signed in!" 
                                
                                if let Some(user) = user.clone() {
                                    div { class: "mt-2",
                                        "User ID: ", span { class: "font-mono", "{user.id}" }
                                        "Email: "
                                        if let Some(email) = &user.email_addresses.first() {
                                            span { class: "font-mono", "{email.email_address}" }
                                        } else {
                                            span { class: "font-mono", "No email" }
                                        }
                                        "Name: " 
                                        span { 
                                            class: "font-mono",
                                            "{user.first_name.as_deref().unwrap_or(\"\")}"
                                            " "
                                            "{user.last_name.as_deref().unwrap_or(\"\")}"
                                        }
                                    }
                                }
                                
                                // Sign out button with loading state
                                match sign_out_result.read().as_ref() {
                                    Some(Ok(_)) => rsx! { "Signed out successfully" },
                                    Some(Err(e)) => rsx! {
                                        div { class: "text-red-500 mb-2", "Error: {e}" }
                                        button {
                                            class: "bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded",
                                            onclick: handle_sign_out,
                                            "Try Again"
                                        }
                                    },
                                    None => rsx! {
                                        button {
                                            class: "bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded",
                                            onclick: handle_sign_out,
                                            "Sign Out"
                                        }
                                    }
                                }
                            }
                        },
                        ClerkStatus::SignedOut => rsx! {
                            div { class: "bg-yellow-100 border border-yellow-400 text-yellow-700 px-4 py-3 rounded mb-4",
                                "You are not signed in."
                            }
                            
                            div { class: "bg-white border rounded p-4",
                                h3 { class: "font-semibold mb-4", "Sign In" }
                                
                                form { 
                                    class: "space-y-4",
                                    onsubmit: handle_sign_in,
                                    
                                    div { class: "mb-4",
                                        label { class: "block text-gray-700 text-sm font-bold mb-2", "Email:" }
                                        input { 
                                            class: "shadow appearance-none border rounded w-full py-2 px-3 text-gray-700 leading-tight focus:outline-none focus:shadow-outline",
                                            r#type: "email",
                                            placeholder: "Email",
                                            value: "{email}",
                                            oninput: move |e| email.set(e.value().clone())
                                        }
                                    }
                                    
                                    div { class: "mb-6",
                                        label { class: "block text-gray-700 text-sm font-bold mb-2", "Password:" }
                                        input { 
                                            class: "shadow appearance-none border rounded w-full py-2 px-3 text-gray-700 mb-3 leading-tight focus:outline-none focus:shadow-outline",
                                            r#type: "password",
                                            placeholder: "******************",
                                            value: "{password}",
                                            oninput: move |e| password.set(e.value().clone())
                                        }
                                    }
                                    
                                    // Handle different states of the sign-in resource
                                    match sign_in_result.read().as_ref() {
                                        Some(Ok(_)) => rsx! {
                                            div { class: "bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded mb-4",
                                                "Sign in successful!"
                                            }
                                        },
                                        Some(Err(e)) => rsx! {
                                            div { class: "bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4",
                                                "Error: {e}"
                                            }
                                        },
                                        None => rsx!{ "" }
                                    }
                                    
                                    button {
                                        class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline",
                                        r#type: "submit",
                                        "Sign In"
                                    }
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
    // Access auth state for conditional rendering
    let is_signed_in = use_is_signed_in();
    let user = use_user();
    
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
                        
                        // Show user info in nav if signed in
                        if is_signed_in {
                            if let Some(user) = user {
                                li { class: "ml-4 flex items-center",
                                    span { class: "text-sm mr-2", "Welcome," }
                                    span { class: "font-semibold", 
                                        if let Some(first_name) = &user.first_name {
                                            if !first_name.is_empty() {
                                                "{first_name}"
                                            } else {
                                                "User"
                                            }
                                        } else {
                                            "User"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}