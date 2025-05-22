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
    clerk.load(false).await?;

    println!("Welcome to the Clerk authentication example!");
    println!("Please select your sign-in method:");
    println!("1. Email Code");
    println!("2. Ticket");

    let choice = read_input("Enter your choice (1 or 2): ");

    match choice.as_str() {
        "1" => {
            // Email Code flow
            let email = read_input("Please enter your email address: ");

            // Create sign-in attempt
            let sign_in_response = clerk
                .get_fapi_client()
                .create_sign_in(
                    None,               // origin
                    Some("email_code"), // strategy
                    Some(&email),       // identifier
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
                .await?;

            let sign_in_id = sign_in_response.response.id;

            println!("We've sent a verification code to your email.");
            println!("Please check your inbox and enter the code below.");

            let code = read_input("Enter verification code: ");

            // Attempt first factor verification
            let verification_response = clerk
                .get_fapi_client()
                .attempt_sign_in_factor_one(
                    &sign_in_id,  // sign_in_id
                    "email_code", // strategy
                    None,         // origin
                    Some(&code),  // code
                    None,         // password
                    None,         // signature
                    None,         // token
                    None,         // ticket
                    None,         //public_key_credential
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
                return Ok(());
            }
        }
        "2" => {
            // Ticket flow
            let ticket = read_input("Please enter your ticket: ");

            let sign_in_response = clerk
                .get_fapi_client()
                .create_sign_in(
                    None,           // origin
                    Some("ticket"), // strategy
                    None,           // identifier
                    None,           // password
                    Some(&ticket),  // ticket
                    None,           // redirect_url
                    None,           // action_complete_redirect_url
                    None,           // transfer
                    None,           // code
                    None,           // token
                    None,           // oidc_login_hint
                    None,           // oidc_prompt
                )
                .await?;

            if sign_in_response.response.status
                == clerk_fapi_rs::models::client_period_sign_in::Status::Complete
            {
                println!("Sign in successful!");
            } else {
                println!(
                    "Sign in failed. Status: {:?}",
                    sign_in_response.response.status
                );
                return Ok(());
            }
        }
        _ => {
            println!("Invalid choice!");
            return Ok(());
        }
    }

    // Give some time for the client to update
    sleep(Duration::from_millis(500)).await;

    // Get and display user information
    if let Some(user) = clerk.user().unwrap() {
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

    let memberships = clerk
        .get_fapi_client()
        .get_organization_memberships(None, None, None)
        .await
        .unwrap();
    println!("\nOrganizations:");

    let data = *memberships.response;

    match data {
        clerk_fapi_rs::models::ClientClientWrappedOrganizationMembershipsResponse::ClientClientWrappedOrganizationMembershipsResponseOneOf(memberships) => {
            let mems = memberships.data.unwrap();
            println!("Found {} memberships (1): ", mems.len());
            for membership in mems {
                let org = membership.organization;
                let name = org.name;
                println!("- Organization: {}", name);
            }
        },
        clerk_fapi_rs::models::ClientClientWrappedOrganizationMembershipsResponse::Array(memberships) => {
            println!("Found {} memberships (2): ", memberships.len());
            for membership in memberships {
                let org = membership.organization;
                let name = org.name;
                println!("- Organization: {}", name);
            }
        },
    };

    Ok(())
}
