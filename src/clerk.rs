use crate::apis::configuration::Configuration as ApiConfiguration;
use crate::clerk_fapi::ClerkFapiClient;
use crate::configuration::ClerkFapiConfiguration;
use crate::models::{
    ClientPeriodClient as Client, ClientPeriodEnvironment as Environment,
    ClientPeriodOrganization as Organization, ClientPeriodSession as Session,
    ClientPeriodUser as User,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, RwLockWriteGuard};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The main client for interacting with Clerk's Frontend API
#[derive(Clone, Default)]
pub struct Clerk {
    config: Arc<ClerkFapiConfiguration>,
    state: Arc<RwLock<ClerkState>>,
    api_client: Arc<ClerkFapiClient>,
    listeners: Arc<Mutex<Vec<ListenerCallback>>>,
}

type ListenerCallback = Box<dyn Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync + 'static>;

#[derive(Default)]
struct ClerkState {
    environment: Option<Environment>,
    client: Option<Client>,
    session: Option<Session>,
    user: Option<User>,
    organization: Option<Organization>,
    loaded: bool,
}

impl Clerk {
    /// Creates a new ClerkFapiClient with the provided configuration
    pub fn new(config: ClerkFapiConfiguration) -> Self {
        let api_client = ClerkFapiClient::new(config.clone()).unwrap();
        let api_client = Arc::new(api_client);

        // Create new Clerk instance
        let clerk = Self {
            config: Arc::new(config),
            state: Arc::new(RwLock::new(ClerkState::default())),
            api_client: api_client.clone(),
            listeners: Arc::new(Mutex::new(Vec::new())),
        };

        // Create and set the callback
        let clerk_clone = clerk.clone();
        let callback = Box::new(move |client| {
            let clerk = clerk_clone.clone();
            Box::pin(async move { clerk.update_client(client).await })
                as Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        });

        // Set the callback on the API client
        api_client.set_update_client_callback(callback);

        clerk
    }

    /// getter for the api_client
    pub fn get_fapi_client(&self) -> &ClerkFapiClient {
        &self.api_client
    }

    /// Returns a reference to the client's configuration
    pub fn config(&self) -> &ClerkFapiConfiguration {
        &self.config
    }

    /// Helper function to load and set the environment
    async fn load_environment(&self) -> Result<(), String> {
        // First check if environment exists in store
        if let Some(stored_env) = self.config.get_store_value("environment") {
            // Try to deserialize the stored environment
            if let Ok(environment) = serde_json::from_value::<Environment>(stored_env) {
                // Update state and store using update_environment
                self.update_environment(environment).await?;

                // Clone what we need for background task
                let api_client = self.api_client.clone();
                let this = self.clone();

                // Spawn background task to update environment
                tokio::spawn(async move {
                    const RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

                    loop {
                        // Try to fetch fresh environment
                        match api_client.get_environment().await {
                            Ok(fresh_env) => {
                                // Update state and store using update_environment
                                if let Err(e) = this.update_environment(fresh_env).await {
                                    eprintln!(
                                        "Failed to update environment in background task: {}",
                                        e
                                    );
                                    continue;
                                }
                                // Success - break the retry loop
                                break;
                            }
                            Err(_) => {
                                // Failed to fetch - wait before retrying
                                tokio::time::sleep(RETRY_INTERVAL).await;
                                continue;
                            }
                        }
                    }
                });

                return Ok(());
            }
        }

        // If no valid environment in store, fetch from API
        let environment = self
            .api_client
            .get_environment()
            .await
            .map_err(|e| format!("Failed to fetch environment: {}", e))?;

        // Update state and store using update_environment
        self.update_environment(environment).await?;

        Ok(())
    }

    /// Helper function to load and set the client
    async fn load_client(&self) -> Result<(), String> {
        // First check if client exists in store
        if let Some(stored_client) = self.config.get_store_value("client") {
            // Try to deserialize the stored client
            if let Ok(client) = serde_json::from_value::<Client>(stored_client) {
                // Update state with stored client
                self.update_client(client).await?;

                // Clone what we need for background task
                let api_client = self.api_client.clone();
                let this = self.clone();

                // Spawn background task to update client
                tokio::spawn(async move {
                    const RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

                    loop {
                        // Try to fetch fresh client
                        match api_client.get_client().await {
                            Ok(fresh_client_response) => {
                                if let Some(Some(fresh_client)) = fresh_client_response.response {
                                    // Update state and store using update_client
                                    if let Err(e) = this.update_client(*fresh_client).await {
                                        eprintln!(
                                            "Failed to update client in background task: {}",
                                            e
                                        );
                                        continue;
                                    }
                                    // Success - break the retry loop
                                    break;
                                }
                            }
                            Err(_) => {
                                // Failed to fetch - wait before retrying
                                tokio::time::sleep(RETRY_INTERVAL).await;
                                continue;
                            }
                        }
                    }
                });

                return Ok(());
            }
        }

        // If no valid client in store, fetch from API
        let client_response = self
            .api_client
            .get_client()
            .await
            .map_err(|e| format!("Failed to fetch client: {}", e))?;

        // Update client state if response contains client data
        if let Some(Some(client)) = client_response.response {
            self.update_client(*client).await?;
        }

        Ok(())
    }

    /// Initialize the client by fetching environment and client data
    ///
    /// This method must be called before using other client methods.
    /// If the client is already loaded, this method returns immediately.
    ///
    /// # Returns
    ///
    /// Returns a Result containing self if successful
    ///
    /// # Errors
    ///
    /// Returns an error if either API call fails
    pub async fn load(self) -> Result<Self, String> {
        // Return early if already loaded
        if self.state.read().await.loaded {
            return Ok(self);
        }

        // Load environment and client concurrently
        let (env_result, client_result) = tokio::join!(self.load_environment(), self.load_client());

        // Check results
        env_result?;
        client_result?;

        // Set loaded flag
        {
            let mut state = self.state.write().await;
            state.loaded = true;
        }

        Ok(self)
    }

    /// Returns whether the client has been initialized
    pub async fn loaded(&self) -> bool {
        self.state.read().await.loaded
    }

    /// Returns the current environment if initialized
    pub async fn environment(&self) -> Option<Environment> {
        self.state.read().await.environment.clone()
    }

    /// Returns the current client if initialized
    pub async fn client(&self) -> Option<Client> {
        self.state.read().await.client.clone()
    }

    /// Returns the current session if set
    pub async fn session(&self) -> Option<Session> {
        self.state.read().await.session.clone()
    }

    /// Returns the current user if set
    pub async fn user(&self) -> Option<User> {
        self.state.read().await.user.clone()
    }

    /// Returns the current organization if set
    pub async fn organization(&self) -> Option<Organization> {
        self.state.read().await.organization.clone()
    }

    /// Updates the client state based on the provided client data
    /// This includes updating the client, session, user, and organization state
    pub async fn update_client(&self, client: Client) -> Result<(), String> {
        let mut state = self.state.write().await;

        // Update client state
        state.client = Some(client.clone());
        let fresh_client = client.clone();

        // Get the active session from the sessions list
        let active_session = client.last_active_session_id.and_then(|id| {
            client
                .sessions
                .iter()
                .find(|s| s.id == Some(id.clone()))
                .cloned()
        });

        // Update session and related state
        self.set_accessors(&mut state, active_session)?;

        // Save client to store
        self.config.set_store_value(
            "client",
            serde_json::to_value(fresh_client.clone())
                .map_err(|e| format!("Failed to serialize client: {}", e))?,
        );

        // Get current state for listeners
        let current_session = state.session.clone();
        let current_user = state.user.clone();
        let current_organization = state.organization.clone();

        // Drop the write lock before notifying listeners
        drop(state);

        // Notify all listeners
        if let Ok(listeners) = self.listeners.lock() {
            for listener in listeners.iter() {
                listener(
                    fresh_client.clone(),
                    current_session.clone(),
                    current_user.clone(),
                    current_organization.clone(),
                );
            }
        }

        Ok(())
    }

    /// Sets the session, user and organization state based on the provided active session
    fn set_accessors<'a>(
        &self,
        state: &mut RwLockWriteGuard<'a, ClerkState>,
        active_session: Option<Session>,
    ) -> Result<(), String> {
        match active_session {
            Some(session) => {
                // Update session state
                state.session = Some(session.clone());

                // Update user state from session
                if let Some(Some(user)) = session.user {
                    state.user = Some(*user.clone());

                    // Find organization from user's memberships
                    if let Some(last_active_org_id) = session.last_active_organization_id {
                        if let Some(ref memberships) = user.organization_memberships {
                            if let Some(Some(active_org)) = memberships
                                .iter()
                                .find(|m| {
                                    m.organization.clone().unwrap().id
                                        == Some(last_active_org_id.clone())
                                })
                                .map(|m| m.organization.clone())
                            {
                                state.organization = Some(*active_org);
                            }
                        }
                    } else {
                        state.organization = None;
                    }
                }
            }
            None => {
                // Clear all state if no active session found
                state.session = None;
                state.user = None;
                state.organization = None;
            }
        }

        Ok(())
    }

    /// Get a session JWT token for the current session
    ///
    /// Returns None if:
    /// - Client is not loaded
    /// - No active session exists
    /// - No user is associated with the session
    /// - Token creation fails
    ///
    /// # Arguments
    ///
    /// * `organization_id` - Optional organization ID to scope the token to
    /// * `template` - Optional template name to use for token creation
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example(client: clerk_fapi_rs::clerk::Clerk) -> Result<(), Box<dyn std::error::Error>> {
    /// let token = client.get_token(None, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_token(
        &self,
        organization_id: Option<&str>,
        template: Option<&str>,
    ) -> Result<Option<String>, String> {
        // Check if client is loaded and has active session
        if !self.loaded().await {
            return Ok(None);
        }

        let session = match self.session().await {
            Some(s) => s,
            None => return Ok(None),
        };

        // Check if session has associated user
        if self.user().await.is_none() {
            return Ok(None);
        }

        // Call appropriate token creation method based on parameters
        let result = match template {
            Some(template_name) => self
                .api_client
                .create_session_token_with_template(&session.id.unwrap(), template_name)
                .await
                .map_err(|e| format!("Failed to create session token with template: {}", e))?,
            None => self
                .api_client
                .create_session_token(&session.id.unwrap(), organization_id)
                .await
                .map_err(|e| format!("Failed to create session token: {}", e))?,
        };

        Ok(result.jwt)
    }

    /// Signs out either a specific session or all sessions for this client
    ///
    /// # Arguments
    ///
    /// * `session_id` - Optional session ID to sign out. If None, signs out all sessions.
    ///
    /// # Returns
    ///
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    ///
    /// Returns an error if the API call fails
    pub async fn sign_out(&self, session_id: Option<String>) -> Result<(), String> {
        match session_id {
            Some(sid) => {
                self.api_client
                    .remove_session(&sid)
                    .await
                    .map_err(|e| format!("Failed to remove session: {}", e))?
                    .client
            }
            None => {
                self.api_client
                    .remove_client_sessions_and_retain_cookie()
                    .await
                    .map_err(|e| format!("Failed to remove all sessions: {}", e))?
                    .client
            }
        };
        // The remove sessions calls will update the client state via the callback

        Ok(())
    }

    /// Updates the active session and/or organization
    ///
    /// # Arguments
    ///
    /// * `session_id` - Optional session ID to set as active
    /// * `organization_id_or_slug` - Optional organization ID or slug to set as active
    ///
    /// # Returns
    ///
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not loaded
    /// - Neither current session nor session_id is available
    /// - Both arguments are None
    /// - Session ID is not found in client sessions
    /// - Organization ID/slug is not found in user's memberships
    pub async fn set_active(
        &self,
        session_id: Option<String>,
        organization_id_or_slug: Option<String>,
    ) -> Result<(), String> {
        // Check if client is loaded
        if !self.loaded().await {
            return Err("Cannot set active session before client is loaded".to_string());
        }

        // Both arguments cannot be None
        if session_id.is_none() && organization_id_or_slug.is_none() {
            return Err(
                "Either session_id or organization_id_or_slug must be provided".to_string(),
            );
        }

        let mut state = self.state.write().await;
        let client = state.client.as_ref().ok_or("Client not found")?;

        // Get the target session either from the argument or current session
        let mut target_session = if let Some(sid) = session_id {
            client
                .sessions
                .iter()
                .find(|s| s.id.as_ref() == Some(&sid))
                .cloned()
                .ok_or_else(|| format!("Session with ID {} not found", sid))?
        } else {
            state
                .session
                .clone()
                .ok_or("No active session and no session_id provided")?
        };

        // Parse the user data from the session if it exists
        let user = match &target_session.user {
            Some(Some(user_value)) => *user_value.clone(),
            _ => return Err("No user data found in session".to_string()),
        };

        // If organization_id_or_slug is provided, update the session's last active organization
        if let Some(org_id_or_slug) = organization_id_or_slug {
            if org_id_or_slug.starts_with("org_") {
                // It's an organization ID - verify it exists in user's memberships
                let org_exists = user
                    .organization_memberships
                    .as_ref()
                    .map(|memberships| {
                        memberships.iter().any(|m| {
                            m.organization
                                .as_ref()
                                .and_then(|o| o.id.as_ref())
                                .map(|id| *id == org_id_or_slug)
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

                if !org_exists {
                    return Err(format!(
                        "Organization with ID '{}' not found in user's memberships",
                        org_id_or_slug
                    ));
                }
                target_session.last_active_organization_id = Some(org_id_or_slug);
            } else {
                // Try to find organization by slug
                let org_id = user
                    .organization_memberships
                    .as_ref()
                    .and_then(|memberships| {
                        memberships.iter().find_map(|m| {
                            if m.organization
                                .as_ref()
                                .and_then(|o| o.slug.as_ref())
                                .map(|slug| *slug == org_id_or_slug)
                                .unwrap_or(false)
                            {
                                m.organization.as_ref().and_then(|o| o.id.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .ok_or_else(|| {
                        format!(
                            "Organization with slug '{}' not found in user's memberships",
                            org_id_or_slug
                        )
                    })?;

                target_session.last_active_organization_id = Some(org_id);
            }
        }

        // Touch the target session using the clerk_fapi client
        if let Some(session_id) = target_session.id.as_ref() {
            self.api_client
                .touch_session(session_id, None)
                .await
                .map_err(|e| format!("Failed to touch session: {}", e))?;
        }

        // Update all state using set_accessors
        self.set_accessors(&mut state, Some(target_session))?;

        Ok(())
    }

    /// Add this new method
    async fn update_environment(&self, environment: Environment) -> Result<(), String> {
        // Update state
        {
            let mut state = self.state.write().await;
            state.environment = Some(environment.clone());
        }

        // Save environment to store
        self.config.set_store_value(
            "environment",
            serde_json::to_value(environment)
                .map_err(|e| format!("Failed to serialize environment: {}", e))?,
        );

        Ok(())
    }

    /// Adds a listener that will be called whenever the client state changes
    /// The listener receives the current Client, Session, User and Organization state
    /// If there's already a loaded client, the callback will be called immediately
    pub fn add_listener<F>(&self, callback: F)
    where
        F: Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync + Clone + 'static,
    {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.push(Box::new(callback.clone()));

        // If we already have a loaded client, call the callback immediately
        if let Ok(state) = self.state.try_read() {
            if let Some(client) = state.client.clone() {
                let session = state.session.clone();
                let user = state.user.clone();
                let organization = state.organization.clone();
                
                // Drop the read lock before calling the callback
                drop(state);
                
                callback(client, session, user, organization);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{
        client_period_account_portal, client_period_active_session, client_period_auth_config,
        client_period_client, client_period_display_config, client_period_email_address,
        client_period_organization, client_period_organization_domain,
        client_period_organization_invitation, client_period_organization_invitation_user_context,
        client_period_organization_membership, client_period_organization_membership_request,
        client_period_organization_suggestion, client_period_passkey, client_period_permission,
        client_period_phone_number, client_period_role, client_period_saml_account,
        client_period_session::{self, Status},
        client_period_session_base, client_period_sign_in, client_period_sign_up,
        client_period_user, client_period_web3_wallet, external_account_with_verification, token,
        ClientPeriodAuthConfig,
    };

    use super::*;
    use mockito::Server;
    use serde_json;

    #[tokio::test]
    async fn test_init() {
        let mut mock_server = mockito::Server::new_async().await;
        let client = serde_json::json!({
                "id": "test_client",
                "object": "client",
                "sign_in": null,
                "sign_up": null,
                "sessions": [],
                "last_active_session_id": null,
                "created_at": 1704067200,
                "updated_at": 1704067200
        });

        let client_mock = mock_server
            .mock("GET", "/v1/client?_is_native=1")
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "response": client,
                    "client": null
                })
                .to_string(),
            )
            .create_async()
            .await;

        let env_mock = mock_server
            .mock("GET", "/v1/environment?_is_native=1")
            .with_status(200)
            .with_body(
                serde_json::json!(
                    {
                        "auth_config": {
                          "object": "auth_config",
                          "id": "aac_asdfasdfasdfasdf",
                          "first_name": "on",
                          "last_name": "on",
                          "email_address": "on",
                          "phone_number": "off",
                          "username": "on",
                          "password": "required",
                          "identification_requirements": [
                            [
                              "email_address",
                              "oauth_google"
                            ],
                            [
                              "username"
                            ]
                          ],
                          "identification_strategies": [
                            "email_address",
                            "oauth_google",
                            "username"
                          ],
                          "first_factors": [
                            "email_code",
                            "email_link",
                            "google_one_tap",
                            "oauth_google",
                            "password",
                            "reset_password_email_code",
                            "ticket"
                          ],
                          "second_factors": [
                            "totp"
                          ],
                          "email_address_verification_strategies": [
                            "email_code"
                          ],
                          "single_session_mode": true,
                          "enhanced_email_deliverability": false,
                          "test_mode": false,
                          "cookieless_dev": false,
                          "url_based_session_syncing": false,
                          "demo": false
                        },
                        "display_config": {
                          "object": "display_config",
                          "id": "display_config_asdfasdfasdf",
                          "instance_environment_type": "production",
                          "application_name": "reconfigured",
                          "theme": {
                            "buttons": {
                              "font_color": "#ffffff",
                              "font_family": "\"Source Sans Pro\", sans-serif",
                              "font_weight": "600"
                            },
                            "general": {
                              "color": "#8C00C7",
                              "padding": "1em",
                              "box_shadow": "0 2px 8px rgba(0, 0, 0, 0.2)",
                              "font_color": "#151515",
                              "font_family": "\"Source Sans Pro\", sans-serif",
                              "border_radius": "0.5em",
                              "background_color": "#ffffff",
                              "label_font_weight": "600"
                            },
                            "accounts": {
                              "background_color": "#ffffff"
                            }
                          },
                          "preferred_sign_in_strategy": "password",
                          "logo_image_url": "",
                          "favicon_image_url": "",
                          "home_url": "",
                          "sign_in_url": "",
                          "sign_up_url": "",
                          "user_profile_url": "",
                          "waitlist_url": "",
                          "after_sign_in_url": "",
                          "after_sign_up_url": "",
                          "after_sign_out_one_url": "",
                          "after_sign_out_all_url": "",
                          "after_switch_session_url": "",
                          "after_join_waitlist_url": "",
                          "organization_profile_url": "",
                          "create_organization_url": "",
                          "after_leave_organization_url": "",
                          "after_create_organization_url": "",
                          "logo_link_url": "",
                          "support_email": "support@reconfigured.io",
                          "branded": false,
                          "experimental_force_oauth_first": false,
                          "clerk_js_version": "5",
                          "show_devmode_warning": false,
                          "google_one_tap_client_id": "",
                          "help_url": null,
                          "privacy_policy_url": "",
                          "terms_url": "",
                          "logo_url": "",
                          "favicon_url": "",
                          "logo_image": {
                            "object": "image",
                            "id": "img_asdfasdf",
                            "public_url": ""
                          },
                          "favicon_image": {
                            "object": "image",
                            "id": "img_asdfasdf",
                            "public_url": ""
                          },
                          "captcha_public_key": "asdf",
                          "captcha_widget_type": "invisible",
                          "captcha_public_key_invisible": "asdf",
                          "captcha_provider": "turnstile",
                          "captcha_oauth_bypass": []
                        },
                        "user_settings": {
                          "attributes": {
                            "email_address": {
                              "enabled": true,
                              "required": true,
                              "used_for_first_factor": true,
                              "first_factors": [
                                "email_code",
                                "email_link"
                              ],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [
                                "email_code"
                              ],
                              "verify_at_sign_up": true
                            },
                            "phone_number": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "username": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": true,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "web3_wallet": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "first_name": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "last_name": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "password": {
                              "enabled": true,
                              "required": true,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "authenticator_app": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": true,
                              "second_factors": [
                                "totp"
                              ],
                              "verifications": [
                                "totp"
                              ],
                              "verify_at_sign_up": false
                            },
                            "ticket": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "backup_code": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "passkey": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            }
                          },
                          "sign_in": {
                            "second_factor": {
                              "required": false
                            }
                          },
                          "sign_up": {
                            "captcha_enabled": true,
                            "captcha_widget_type": "invisible",
                            "custom_action_required": false,
                            "progressive": true,
                            "mode": "public",
                            "legal_consent_enabled": true
                          },
                          "restrictions": {
                            "allowlist": {
                              "enabled": false
                            },
                            "blocklist": {
                              "enabled": false
                            },
                            "block_email_subaddresses": {
                              "enabled": true
                            },
                            "block_disposable_email_domains": {
                              "enabled": true
                            },
                            "ignore_dots_for_gmail_addresses": {
                              "enabled": true
                            }
                          },
                          "username_settings": {
                            "min_length": 4,
                            "max_length": 64
                          },
                          "actions": {
                            "delete_self": true,
                            "create_organization": true,
                            "create_organizations_limit": 3
                          },
                          "attack_protection": {
                            "user_lockout": {
                              "enabled": true,
                              "max_attempts": 100,
                              "duration_in_minutes": 60
                            },
                            "pii": {
                              "enabled": true
                            },
                            "email_link": {
                              "require_same_client": false
                            }
                          },
                          "passkey_settings": {
                            "allow_autofill": true,
                            "show_sign_in_button": true
                          },
                          "social": {
                            "oauth_google": {
                              "enabled": true,
                              "required": false,
                              "authenticatable": true,
                              "block_email_subaddresses": true,
                              "strategy": "oauth_google",
                              "not_selectable": false,
                              "deprecated": false,
                              "name": "Google",
                              "logo_url": "https://img.clerk.com/static/google.png"
                            },
                            "oauth_microsoft": {
                              "enabled": false,
                              "required": false,
                              "authenticatable": false,
                              "block_email_subaddresses": false,
                              "strategy": "oauth_microsoft",
                              "not_selectable": false,
                              "deprecated": false,
                              "name": "Microsoft",
                              "logo_url": "https://img.clerk.com/static/microsoft.png"
                            }
                          },
                          "password_settings": {
                            "disable_hibp": false,
                            "min_length": 0,
                            "max_length": 0,
                            "require_special_char": false,
                            "require_numbers": false,
                            "require_uppercase": false,
                            "require_lowercase": false,
                            "show_zxcvbn": false,
                            "min_zxcvbn_strength": 0,
                            "enforce_hibp_on_sign_in": false,
                            "allowed_special_characters": "!\"#$%&'()*+,-./:;<=>?@[]^_`{|}~"
                          },
                          "saml": {
                            "enabled": false
                          },
                          "enterprise_sso": {
                            "enabled": false
                          }
                        },
                        "organization_settings": {
                          "enabled": true,
                          "max_allowed_memberships": 5,
                          "actions": {
                            "admin_delete": true
                          },
                          "domains": {
                            "enabled": false,
                            "enrollment_modes": [],
                            "default_role": "org:member"
                          },
                          "creator_role": "org:admin"
                        },
                        "maintenance_mode": false
                      }
                )
                .to_string(),
            )
            .create_async()
            .await;

        let clerk = Clerk::new(
            ClerkFapiConfiguration::new(
                "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
                Some(mock_server.url()),
                None,
            )
            .unwrap(),
        );

        let result = clerk.clone().load().await.unwrap();

        env_mock.assert_async().await;
        client_mock.assert_async().await;
        assert!(result.environment().await.is_some());
    }

    #[tokio::test]
    async fn test_init_environment_failure() {
        let mut server = Server::new_async().await;

        // Mock failed environment endpoint with /v1 prefix
        let env_mock = server
            .mock("GET", "/v1/client?_is_native=1")
            .with_status(500)
            .create_async()
            .await;

        let config = ClerkFapiConfiguration::new(
            "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
            Some(server.url()),
            None,
        )
        .unwrap();

        let client = Clerk::new(config);

        // Test initialization fails
        let result = client.load().await;
        assert!(result.is_err());

        // Verify the mock was called
        env_mock.assert_async().await;
    }

    #[test]
    fn test_client_cloning() {
        let config =
            ClerkFapiConfiguration::new("pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(), None, None)
                .unwrap();

        let client = Clerk::new(config);
        let cloned_client = client.clone();

        // Verify both clients point to the same configuration
        assert_eq!(
            client.config().base_url(),
            cloned_client.config().base_url()
        );
    }

    #[tokio::test]
    async fn test_init_uses_update_client() {
        let mut server = Server::new_async().await;

        // Mock the environment endpoint with /v1 prefix
        let env_mock = server
            .mock("GET", "/v1/environment?_is_native=1")
            .with_status(200)
            .with_body(
                serde_json::json!(
                    {
                        "auth_config": {
                          "object": "auth_config",
                          "id": "aac_asdfasdfasdfasdf",
                          "first_name": "on",
                          "last_name": "on",
                          "email_address": "on",
                          "phone_number": "off",
                          "username": "on",
                          "password": "required",
                          "identification_requirements": [
                            [
                              "email_address",
                              "oauth_google"
                            ],
                            [
                              "username"
                            ]
                          ],
                          "identification_strategies": [
                            "email_address",
                            "oauth_google",
                            "username"
                          ],
                          "first_factors": [
                            "email_code",
                            "email_link",
                            "google_one_tap",
                            "oauth_google",
                            "password",
                            "reset_password_email_code",
                            "ticket"
                          ],
                          "second_factors": [
                            "totp"
                          ],
                          "email_address_verification_strategies": [
                            "email_code"
                          ],
                          "single_session_mode": true,
                          "enhanced_email_deliverability": false,
                          "test_mode": false,
                          "cookieless_dev": false,
                          "url_based_session_syncing": false,
                          "demo": false
                        },
                        "display_config": {
                          "object": "display_config",
                          "id": "display_config_asdfasdfasdf",
                          "instance_environment_type": "production",
                          "application_name": "reconfigured",
                          "theme": {
                            "buttons": {
                              "font_color": "#ffffff",
                              "font_family": "\"Source Sans Pro\", sans-serif",
                              "font_weight": "600"
                            },
                            "general": {
                              "color": "#8C00C7",
                              "padding": "1em",
                              "box_shadow": "0 2px 8px rgba(0, 0, 0, 0.2)",
                              "font_color": "#151515",
                              "font_family": "\"Source Sans Pro\", sans-serif",
                              "border_radius": "0.5em",
                              "background_color": "#ffffff",
                              "label_font_weight": "600"
                            },
                            "accounts": {
                              "background_color": "#ffffff"
                            }
                          },
                          "preferred_sign_in_strategy": "password",
                          "logo_image_url": "",
                          "favicon_image_url": "",
                          "home_url": "",
                          "sign_in_url": "",
                          "sign_up_url": "",
                          "user_profile_url": "",
                          "waitlist_url": "",
                          "after_sign_in_url": "",
                          "after_sign_up_url": "",
                          "after_sign_out_one_url": "",
                          "after_sign_out_all_url": "",
                          "after_switch_session_url": "",
                          "after_join_waitlist_url": "",
                          "organization_profile_url": "",
                          "create_organization_url": "",
                          "after_leave_organization_url": "",
                          "after_create_organization_url": "",
                          "logo_link_url": "",
                          "support_email": "support@reconfigured.io",
                          "branded": false,
                          "experimental_force_oauth_first": false,
                          "clerk_js_version": "5",
                          "show_devmode_warning": false,
                          "google_one_tap_client_id": "",
                          "help_url": null,
                          "privacy_policy_url": "",
                          "terms_url": "",
                          "logo_url": "",
                          "favicon_url": "",
                          "logo_image": {
                            "object": "image",
                            "id": "img_asdfasdf",
                            "public_url": ""
                          },
                          "favicon_image": {
                            "object": "image",
                            "id": "img_asdfasdf",
                            "public_url": ""
                          },
                          "captcha_public_key": "asdf",
                          "captcha_widget_type": "invisible",
                          "captcha_public_key_invisible": "asdf",
                          "captcha_provider": "turnstile",
                          "captcha_oauth_bypass": []
                        },
                        "user_settings": {
                          "attributes": {
                            "email_address": {
                              "enabled": true,
                              "required": true,
                              "used_for_first_factor": true,
                              "first_factors": [
                                "email_code",
                                "email_link"
                              ],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [
                                "email_code"
                              ],
                              "verify_at_sign_up": true
                            },
                            "phone_number": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "username": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": true,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "web3_wallet": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "first_name": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "last_name": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "password": {
                              "enabled": true,
                              "required": true,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "authenticator_app": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": true,
                              "second_factors": [
                                "totp"
                              ],
                              "verifications": [
                                "totp"
                              ],
                              "verify_at_sign_up": false
                            },
                            "ticket": {
                              "enabled": true,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "backup_code": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            },
                            "passkey": {
                              "enabled": false,
                              "required": false,
                              "used_for_first_factor": false,
                              "first_factors": [],
                              "used_for_second_factor": false,
                              "second_factors": [],
                              "verifications": [],
                              "verify_at_sign_up": false
                            }
                          },
                          "sign_in": {
                            "second_factor": {
                              "required": false
                            }
                          },
                          "sign_up": {
                            "captcha_enabled": true,
                            "captcha_widget_type": "invisible",
                            "custom_action_required": false,
                            "progressive": true,
                            "mode": "public",
                            "legal_consent_enabled": true
                          },
                          "restrictions": {
                            "allowlist": {
                              "enabled": false
                            },
                            "blocklist": {
                              "enabled": false
                            },
                            "block_email_subaddresses": {
                              "enabled": true
                            },
                            "block_disposable_email_domains": {
                              "enabled": true
                            },
                            "ignore_dots_for_gmail_addresses": {
                              "enabled": true
                            }
                          },
                          "username_settings": {
                            "min_length": 4,
                            "max_length": 64
                          },
                          "actions": {
                            "delete_self": true,
                            "create_organization": true,
                            "create_organizations_limit": 3
                          },
                          "attack_protection": {
                            "user_lockout": {
                              "enabled": true,
                              "max_attempts": 100,
                              "duration_in_minutes": 60
                            },
                            "pii": {
                              "enabled": true
                            },
                            "email_link": {
                              "require_same_client": false
                            }
                          },
                          "passkey_settings": {
                            "allow_autofill": true,
                            "show_sign_in_button": true
                          },
                          "social": {
                            "oauth_google": {
                              "enabled": true,
                              "required": false,
                              "authenticatable": true,
                              "block_email_subaddresses": true,
                              "strategy": "oauth_google",
                              "not_selectable": false,
                              "deprecated": false,
                              "name": "Google",
                              "logo_url": "https://img.clerk.com/static/google.png"
                            },
                            "oauth_microsoft": {
                              "enabled": false,
                              "required": false,
                              "authenticatable": false,
                              "block_email_subaddresses": false,
                              "strategy": "oauth_microsoft",
                              "not_selectable": false,
                              "deprecated": false,
                              "name": "Microsoft",
                              "logo_url": "https://img.clerk.com/static/microsoft.png"
                            }
                          },
                          "password_settings": {
                            "disable_hibp": false,
                            "min_length": 0,
                            "max_length": 0,
                            "require_special_char": false,
                            "require_numbers": false,
                            "require_uppercase": false,
                            "require_lowercase": false,
                            "show_zxcvbn": false,
                            "min_zxcvbn_strength": 0,
                            "enforce_hibp_on_sign_in": false,
                            "allowed_special_characters": "!\"#$%&'()*+,-./:;<=>?@[]^_`{|}~"
                          },
                          "saml": {
                            "enabled": false
                          },
                          "enterprise_sso": {
                            "enabled": false
                          }
                        },
                        "organization_settings": {
                          "enabled": true,
                          "max_allowed_memberships": 5,
                          "actions": {
                            "admin_delete": true
                          },
                          "domains": {
                            "enabled": false,
                            "enrollment_modes": [],
                            "default_role": "org:member"
                          },
                          "creator_role": "org:admin"
                        },
                        "maintenance_mode": false
                      }
                )
                .to_string(),
            )
            .create_async()
            .await;

        // Mock the client endpoint with /v1 prefix
        let client_mock = server
            .mock("GET", "/v1/client?_is_native=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::json!({
                "response": {
                  "object": "client",
                  "id": "client_xyz789abcdef123456",
                  "sessions": [
                    {
                      "object": "session",
                      "id": "sess_abc123xyz456def789",
                      "status": "active",
                      "expire_at": 1731932703435i64,
                      "abandon_at": 1733919903435i64,
                      "last_active_at": 1731327903435i64,
                      "last_active_organization_id": "org_987zyx654wvu321",
                      "actor": null,
                      "user": {
                        "id": "user_123abc456def789",
                        "object": "user",
                        "username": "johndoe",
                        "first_name": "John",
                        "last_name": "Doe",
                        "image_url": "https://example.com/images/xyz789.jpg",
                        "has_image": true,
                        "primary_email_address_id": "idn_456def789abc123",
                        "primary_phone_number_id": null,
                        "primary_web3_wallet_id": null,
                        "password_enabled": false,
                        "two_factor_enabled": false,
                        "totp_enabled": false,
                        "backup_code_enabled": false,
                        "email_addresses": [
                          {
                            "id": "idn_456def789abc123",
                            "object": "email_address",
                            "email_address": "john.doe@example.com",
                            "reserved": false,
                            "verification": {
                              "status": "verified",
                              "strategy": "from_oauth_google",
                              "attempts": null,
                              "expire_at": null
                            },
                            "linked_to": [
                              {
                                "type": "oauth_google",
                                "id": "idn_789xyz123abc456"
                              }
                            ],
                            "created_at": 1717411902327i64,
                            "updated_at": 1717411902402i64
                          }
                        ],
                        "phone_numbers": [],
                        "web3_wallets": [],
                        "passkeys": [],
                        "external_accounts": [
                          {
                            "object": "google_account",
                            "id": "idn_789xyz123abc456",
                            "google_id": "987654321012345678901",
                            "approved_scopes": "email https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile openid profile",
                            "email_address": "john.doe@example.com",
                            "given_name": "John",
                            "family_name": "Doe",
                            "picture": "https://example.com/photos/abc123.jpg",
                            "username": "",
                            "public_metadata": {},
                            "label": null,
                            "created_at": 1717411902313i64,
                            "updated_at": 1730105981619i64,
                            "verification": {
                              "status": "verified",
                              "strategy": "oauth_google",
                              "attempts": null,
                              "expire_at": 1717412499056i64
                            }
                          }
                        ],
                        "saml_accounts": [],
                        "public_metadata": {},
                        "unsafe_metadata": {},
                        "external_id": null,
                        "last_sign_in_at": 1731327903443i64,
                        "banned": false,
                        "locked": false,
                        "lockout_expires_in_seconds": null,
                        "verification_attempts_remaining": 100,
                        "created_at": 1717411902366i64,
                        "updated_at": 1731327903477i64,
                        "delete_self_enabled": true,
                        "create_organization_enabled": true,
                        "last_active_at": 1731304721325i64,
                        "mfa_enabled_at": null,
                        "mfa_disabled_at": null,
                        "legal_accepted_at": null,
                        "profile_image_url": "https://example.com/profiles/def456.jpg",
                        "organization_memberships": [
                          {
                            "object": "organization_membership",
                            "id": "orgmem_123xyz789abc456",
                            "public_metadata": {},
                            "role": "org:admin",
                            "role_name": "Admin",
                            "permissions": [
                              "org:sys_profile:manage",
                              "org:sys_profile:delete",
                              "org:sys_memberships:read",
                              "org:sys_memberships:manage",
                              "org:sys_domains:read",
                              "org:sys_domains:manage"
                            ],
                            "created_at": 1729249255195i64,
                            "updated_at": 1729249255195i64,
                            "organization": {
                              "object": "organization",
                              "id": "org_456abc789xyz123",
                              "name": "Example Corp",
                              "slug": "example-corp",
                              "image_url": "https://example.com/logos/ghi789.jpg",
                              "has_image": false,
                              "members_count": 3,
                              "pending_invitations_count": 0,
                              "max_allowed_memberships": 5,
                              "admin_delete_enabled": true,
                              "public_metadata": {},
                              "created_at": 1728747692625i64,
                              "updated_at": 1729510267568i64,
                              "logo_url": null
                            }
                          },
                          {
                            "object": "organization_membership",
                            "id": "orgmem_789def123xyz456",
                            "public_metadata": {},
                            "role": "org:admin",
                            "role_name": "Admin",
                            "permissions": [
                              "org:sys_profile:manage",
                              "org:sys_profile:delete",
                              "org:sys_memberships:read",
                              "org:sys_memberships:manage",
                              "org:sys_domains:read",
                              "org:sys_domains:manage"
                            ],
                            "created_at": 1727879689810i64,
                            "updated_at": 1727879689810i64,
                            "organization": {
                              "object": "organization",
                              "id": "org_xyz456abc789def123",
                              "name": "Test Company",
                              "slug": "test-company",
                              "image_url": "https://example.com/logos/jkl012.jpg",
                              "has_image": true,
                              "members_count": 1,
                              "pending_invitations_count": 0,
                              "max_allowed_memberships": 5,
                              "admin_delete_enabled": true,
                              "public_metadata": {
                                "reconfOrgId": "def456xyz789abc123"
                              },
                              "created_at": 1727879689780i64,
                              "updated_at": 1727879715183i64,
                              "logo_url": "https://example.com/logos/mno345.jpg"
                            }
                          }
                        ]
                      },
                      "public_user_data": {
                        "first_name": "John",
                        "last_name": "Doe",
                        "image_url": "https://example.com/images/pqr678.jpg",
                        "has_image": true,
                        "identifier": "john.doe@example.com",
                        "profile_image_url": "https://example.com/profiles/stu901.jpg"
                      },
                      "created_at": 1731327903443i64,
                      "updated_at": 1731327903495i64,
                      "last_active_token": {
                        "object": "token",
                        "jwt": "eyJrandomJwtTokenXyz789Abc123Def456..."
                      }
                    }
                  ],
                  "sign_in": null,
                  "sign_up": null,
                  "last_active_session_id": "sess_abc123xyz456def789",
                  "cookie_expires_at": null,
                  "created_at": 1731327798987i64,
                  "updated_at": 1731327903492i64
                },
                "client": null
              }).to_string())
            .create_async()
            .await;

        let config = ClerkFapiConfiguration::new(
            "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
            Some(server.url()),
            None,
        )
        .unwrap();

        let client = Clerk::new(config);
        let initialized_client = client.load().await.unwrap();

        // Verify all mocks were called
        env_mock.assert_async().await;
        client_mock.assert_async().await;

        // Verify all state was set
        assert!(initialized_client.loaded().await);
        assert!(initialized_client.environment().await.is_some());
        assert!(initialized_client.client().await.is_some());
        assert!(initialized_client.session().await.is_some());
        assert!(initialized_client.user().await.is_some());
    }

    #[tokio::test]
    async fn test_get_token() {
        let mut server = Server::new_async().await;

        // Mock the token endpoint
        let token_mock = server
            .mock("POST", "/v1/client/sessions/sess_123/tokens?_is_native=1")
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "jwt": "test.jwt.token"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let config = ClerkFapiConfiguration::new(
            "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
            Some(server.url()),
            None,
        )
        .unwrap();

        let client = Clerk::new(config);

        // Manually set up client state for testing
        {
            let mut state = client.state.write().await;
            state.loaded = true;
            state.session = Some(Session {
                id: Some("sess_123".to_string()),
                ..Default::default()
            });
            state.user = Some(User::default());
        }

        // Test successful token creation
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, Some("test.jwt.token".to_string()));
        token_mock.assert_async().await;

        // Test with unloaded client
        {
            let mut state = client.state.write().await;
            state.loaded = false;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);

        // Test with no session
        {
            let mut state = client.state.write().await;
            state.loaded = true;
            state.session = None;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);

        // Test with no user
        {
            let mut state = client.state.write().await;
            state.session = Some(Session {
                id: Some("sess_123".to_string()),
                ..Default::default()
            });
            state.user = None;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn test_listener() {
        let config = ClerkFapiConfiguration::new(
            "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
            None,
            None,
        )
        .unwrap();

        let clerk = Clerk::new(config);
        let was_called = Arc::new(AtomicBool::new(false));
        let was_called_clone = was_called.clone();

        // Add a listener
        clerk.add_listener(move |client, session, user, org| {
            assert_eq!(client.id, Some("test_client".to_string()));
            assert!(session.is_some());
            assert!(user.is_some());
            assert!(org.is_none());
            was_called_clone.store(true, Ordering::SeqCst);
        });

        // Create test data
        let test_client = Client {
            id: Some("test_client".to_string()),
            sessions: vec![Session {
                id: Some("test_session".to_string()),
                user: Some(Some(Box::new(User::default()))),
                ..Default::default()
            }],
            last_active_session_id: Some("test_session".to_string()),
            ..Default::default()
        };

        // Update client which should trigger listener
        clerk.update_client(test_client).await.unwrap();

        // Verify listener was called
        assert!(was_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_listener_immediate_callback() {
        let config = ClerkFapiConfiguration::new(
            "pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(),
            None,
            None,
        )
        .unwrap();

        let clerk = Clerk::new(config);
        
        // Set up initial state
        let test_client = Client {
            id: Some("test_client".to_string()),
            sessions: vec![Session {
                id: Some("test_session".to_string()),
                user: Some(Some(Box::new(User::default()))),
                ..Default::default()
            }],
            last_active_session_id: Some("test_session".to_string()),
            ..Default::default()
        };

        // Update client before adding listener
        clerk.update_client(test_client).await.unwrap();

        let was_called = Arc::new(AtomicBool::new(false));
        let was_called_clone = was_called.clone();

        // Add a listener - should be called immediately
        clerk.add_listener(move |client, session, user, org| {
            assert_eq!(client.id, Some("test_client".to_string()));
            assert!(session.is_some());
            assert!(user.is_some());
            assert!(org.is_none());
            was_called_clone.store(true, Ordering::SeqCst);
        });

        // Verify listener was called immediately
        assert!(was_called.load(Ordering::SeqCst));
    }
}
