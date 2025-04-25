use clerk_fapi_rs::{clerk::Clerk, configuration::ClerkFapiConfiguration};
use dotenv::dotenv;
use std::time::Duration;
use std::{
    env,
    io::{self, Write},
};
use tokio::time::sleep;

fn read_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Get the PUBLIC_KEY from environment variables
    let public_key =
        env::var("CLERK_PUBLISHABLE_KEY").expect("PUBLIC_KEY environment variable is required");

    // Create configuration
    let config = ClerkFapiConfiguration::new(
        public_key, // String
        None,       // No proxy
        None,       // No special domain
    )?;

    // Initialize Clerk client
    let clerk = Clerk::new(config);

    // Load the client (this fetches initial data)
    let clerk = clerk.load().await?;

    println!("Welcome to the Clerk authentication example!");

    let email = read_input("Please enter your email address: ");

    // Create sign-in attempt
    let sign_up_response = clerk
        .get_fapi_client()
        .create_sign_ups(
            None,               // origin
            None,               // transfer,
            None,               // password
            None,               // first_name
            None,               // last_name
            None,               // username
            Some(&email),       // email_address
            None,               // phone_number
            None,               // email_address_or_phone_number
            None,               // unsafe_metadata
            Some("email_code"), // strategy
            None,               // action_complete_redirect_url
            None,               // redirect_url
            None,               // ticket
            None,               // web3_wallet
            None,               // token
            None,               // code
            None,               // captcha_token
            None,               // captcha_error
            None,               // captcha_widget_type
            Some(true),         // legal_accepted,
            None,               // oidc_login_hint
            None,               // oidc_prompt
        )
        .await?;

    let sign_up_id = sign_up_response.response.id;

    println!("We've sent a verification code to your email.");
    println!("Please check your inbox and enter the code below.");

    let code = read_input("Enter verification code: ");

    // Attempt first factor verification
    let verification_response = clerk
        .get_fapi_client()
        .attempt_sign_ups_verification(
            &sign_up_id,        // sign_up_id
            None,               // origin,
            Some("email_code"), // strategy
            Some(&code),        // code
            None,               // signarure
            None,               // token
        )
        .await?;

    if verification_response.response.status
        == clerk_fapi_rs::models::client_period_sign_up::Status::Complete
    {
        println!("Sign up successful!");
    } else {
        println!(
            "Sign up failed. Status: {:?}",
            verification_response.response.status
        );
        return Ok(());
    }

    // Give some time for the client to update
    sleep(Duration::from_millis(500)).await;

    // Get and display user information
    if let Some(user) = clerk.user() {
        println!("\nUser Information:");
        println!(
            "Name: {:?} {:?}",
            user.first_name.unwrap_or_default(),
            user.last_name.unwrap_or_default()
        );

        if !user.email_addresses.is_empty() {
            println!("Email: {}", user.email_addresses[0].email_address);
        }
    } else {
        println!("Could not retrieve user information");
    }

    Ok(())
}
