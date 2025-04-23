use crate::apis::configuration::Configuration as ApiConfiguration;
use crate::clerk_fapi::ClerkFapiClient;
use crate::configuration::ClerkFapiConfiguration;
use crate::models::{
    ClientPeriodClient as Client, ClientPeriodEnvironment as Environment,
    ClientPeriodOrganization as Organization, ClientPeriodSession as Session,
    ClientPeriodUser as User,
};
use parking_lot::{RwLock, RwLockWriteGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub type Listener =
    Arc<dyn Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync>;

/// The main client for interacting with Clerk's Frontend API
#[derive(Clone, Default)]
pub struct Clerk {
    config: Arc<ClerkFapiConfiguration>,
    state: Arc<RwLock<ClerkState>>,
    api_client: Arc<ClerkFapiClient>,
    listeners: Arc<RwLock<Vec<Listener>>>,
}

#[derive(Default)]
struct ClerkState {
    environment: Option<Environment>,
    client: Option<Client>,
    session: Option<Session>,
    user: Option<User>,
    organization: Option<Organization>,
    loaded: bool,
    target_organization_id: Option<Option<String>>,
}

impl Clerk {
    /// Creates a new Clerk client with the provided configuration
    ///
    /// This constructor initializes a new client with the given configuration,
    /// setting up the necessary internal state and API client for interacting
    /// with Clerk's Frontend API.
    pub fn new(config: ClerkFapiConfiguration) -> Self {
        // Create the api_client first without Arc
        let mut api_client = ClerkFapiClient::new(config.clone()).unwrap();

        // Create new Clerk instance
        let mut clerk = Self {
            config: Arc::new(config),
            state: Arc::new(RwLock::new(ClerkState::default())),
            api_client: Arc::new(api_client.clone()),
            listeners: Arc::new(RwLock::new(Vec::new())),
        };

        // Create and set the callback
        let clerk_ref = clerk.clone();
        api_client.set_update_client_callback(move |client| {
            let _ = clerk_ref.update_client(client);
        });

        // Now wrap the configured api_client in Arc
        clerk.api_client = Arc::new(api_client);

        clerk
    }

    /// Returns a reference to the internal Frontend API client
    ///
    /// This method provides access to the underlying API client, allowing
    /// direct interaction with the Clerk API when needed.
    pub fn get_fapi_client(&self) -> &ClerkFapiClient {
        &self.api_client
    }

    /// Returns a reference to the client's configuration
    ///
    /// Provides access to the configuration used by this client,
    /// allowing inspection of settings like base URL and API key.
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
                self.update_environment(environment)?;
                return Ok(());
            }
        }

        self.reload_environment().await
    }

    /// Reloads the environment data from the Clerk API
    ///
    /// This method fetches fresh environment data from the API and
    /// updates the client's state, overwriting any cached data.
    pub async fn reload_environment(&self) -> Result<(), String> {
        // Fetch environment from API
        let environment = self
            .api_client
            .get_environment()
            .await
            .map_err(|e| format!("Failed to fetch environment: {}", e))?;

        // Update state and store using update_environment
        self.update_environment(environment)?;
        Ok(())
    }

    /// Helper function to load and set the client
    async fn load_client(&self) -> Result<(), String> {
        // First check if client exists in store
        if let Some(stored_client) = self.config.get_store_value("client") {
            // Try to deserialize the stored client
            if let Ok(client) = serde_json::from_value::<Client>(stored_client) {
                // Update state with stored client
                self.update_client(client)?;
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
        if let Some(client) = client_response.response {
            self.update_client(*client)?;
        }

        Ok(())
    }

    /// Initialize the client by fetching environment and client data
    ///
    /// This method must be called before using other client methods.
    /// It fetches the environment configuration and client data from the Clerk API.
    /// If the client is already loaded, this method returns immediately.
    ///
    /// # Returns
    /// Returns a Result containing self if successful
    ///
    /// # Errors
    /// Returns an error if either API call fails
    pub async fn load(&self) -> Result<Self, String> {
        // Return early if already loaded
        if self.state.read().loaded {
            return Ok(self.clone());
        }

        self.load_environment().await?;
        self.load_client().await?;

        // Set loaded flag
        {
            let mut state = self.state.write();
            state.loaded = true;
        }

        Ok(self.clone())
    }

    /// Returns whether the client has been initialized
    ///
    /// Checks if the client has successfully loaded environment and client data.
    /// This can be used to determine if the `load()` method has been called successfully.
    pub fn loaded(&self) -> bool {
        self.state.read().loaded
    }

    /// Returns the current environment if initialized
    ///
    /// Provides access to the Clerk environment data, which includes authentication
    /// configuration, display settings, and other environment-specific information.
    /// Returns None if the client hasn't been loaded yet.
    pub fn environment(&self) -> Option<Environment> {
        self.state.read().environment.clone()
    }

    /// Returns the current client data if initialized
    ///
    /// Provides access to the Clerk client data, which includes information about
    /// the current browser/device client and its associated sessions.
    /// Returns None if the client hasn't been loaded yet.
    pub fn client(&self) -> Option<Client> {
        self.state.read().client.clone()
    }

    /// Returns the current active session if available
    ///
    /// Provides access to the user's active session, which contains authentication
    /// state and session-specific data. Returns None if no active session exists
    /// or if the client hasn't been loaded yet.
    pub fn session(&self) -> Option<Session> {
        self.state.read().session.clone()
    }

    /// Returns the current authenticated user if available
    ///
    /// Provides access to the authenticated user associated with the active session.
    /// Returns user data including profile information, email addresses, and organization memberships.
    /// Returns None if no user is authenticated or if the client hasn't been loaded yet.
    pub fn user(&self) -> Option<User> {
        self.state.read().user.clone()
    }

    /// Returns the active organization if available
    ///
    /// Provides access to the currently active organization for the authenticated user.
    /// This is the organization that was last activated in the session, or was specified
    /// with `set_active()`. Returns None if no organization is active or if the client
    /// hasn't been loaded yet.
    pub fn organization(&self) -> Option<Organization> {
        self.state.read().organization.clone()
    }

    /// Notifies all registered listeners with the current state
    fn notify_listeners(&self) {
        let client_opt;
        let current_session;
        let current_user;
        let current_organization;

        {
            let state = self.state.read();
            if state.client.is_none() {
                return;
            }
            client_opt = state.client.clone();
            current_session = state.session.clone();
            current_user = state.user.clone();
            current_organization = state.organization.clone();
        }

        if let Some(client) = client_opt {
            let listeners = {
                self.listeners.read().clone() // cheap Arc clones
            };
            for listener in listeners.iter() {
                let client_clone = client.clone();
                let session_clone = current_session.clone();
                let user_clone = current_user.clone();
                let org_clone = current_organization.clone();
                listener(client_clone, session_clone, user_clone, org_clone);
            }
        }
    }

    /// Updates the client state based on the provided client data
    ///
    /// This method updates the internal state with new client data, which includes
    /// extracting and updating the session, user, and organization state as well.
    /// It also saves the client data to the store and notifies any registered listeners.
    ///
    /// # Arguments
    /// * `client` - The new client data to update state with
    ///
    /// # Returns
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    /// Returns an error if serialization of client data fails
    pub fn update_client(&self, client: Client) -> Result<(), String> {
        // Get the active session from the sessions list
        let client_clone = client.clone();
        let active_session = client_clone.last_active_session_id.as_ref().and_then(|id| {
            client_clone
                .sessions
                .iter()
                .find(|s| s.id == id.clone())
                .cloned()
        });

        {
            let mut state = self.state.write();
            state.client = Some(client.clone());

            // Remove mut self requirement from set_accessors
            Self::set_accessors(&mut state, active_session)?;
        }

        // Save client to store (do this outside the lock to avoid holding lock during I/O)
        let fresh_client = client.clone();
        self.config.set_store_value(
            "client",
            serde_json::to_value(fresh_client)
                .map_err(|e| format!("Failed to serialize client: {}", e))?,
        );

        self.notify_listeners();
        Ok(())
    }

    /// Sets the session, user and organization state based on the provided active session
    fn set_accessors(
        state: &mut RwLockWriteGuard<ClerkState>,
        active_session: Option<Session>,
    ) -> Result<(), String> {
        match active_session {
            Some(session) => {
                // Update session state
                state.session = Some(session.clone());

                // Update user state from session
                if let Some(user) = session.user {
                    state.user = Some(*user.clone());
                    let target_org_id = state.target_organization_id.clone();

                    let org_id_target = if let Some(org_id) = target_org_id {
                        org_id
                    } else {
                        session.last_active_organization_id
                    };

                    // We've used the value --> time to reset
                    state.target_organization_id = None;

                    // Find organization from user's memberships
                    if let Some(last_active_org_id) = org_id_target {
                        if let Some(ref memberships) = user.organization_memberships {
                            if let Some(active_org) = memberships
                                .iter()
                                .find(|m| m.organization.id == last_active_org_id.clone())
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
    /// Creates and returns a JWT token for the current active session. The token can be
    /// optionally scoped to an organization or created with a specific template.
    ///
    /// Returns None if:
    /// - Client is not loaded
    /// - No active session exists
    /// - No user is associated with the session
    /// - Token creation fails
    ///
    /// # Arguments
    /// * `organization_id` - Optional organization ID to scope the token to
    /// * `template` - Optional template name to use for token creation
    ///
    /// # Returns
    /// Returns a Result containing an Option<String>. The string contains the JWT token
    /// if successful, or None if no token could be created.
    ///
    /// # Errors
    /// Returns an error if the API call fails
    ///
    /// # Examples
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
        if !self.loaded() {
            return Ok(None);
        }

        let session = match self.session() {
            Some(s) => s,
            None => return Ok(None),
        };

        // Check if session has associated user
        if self.user().is_none() {
            return Ok(None);
        }

        // Call appropriate token creation method based on parameters
        let result = match template {
            Some(template_name) => self
                .api_client
                .create_session_token_with_template(&session.id, template_name)
                .await
                .map_err(|e| format!("Failed to create session token with template: {}", e))?,
            None => self
                .api_client
                .create_session_token(&session.id, organization_id)
                .await
                .map_err(|e| format!("Failed to create session token: {}", e))?,
        };

        Ok(result.jwt)
    }

    /// Signs out either a specific session or all sessions for this client
    ///
    /// This method allows signing out a single session by ID, or signing out all sessions
    /// for the current client if no session ID is provided. After successful sign-out,
    /// the client state will be updated accordingly via the callback mechanism.
    ///
    /// # Arguments
    /// * `session_id` - Optional session ID to sign out. If None, signs out all sessions.
    ///
    /// # Returns
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    /// Returns an error if the API call fails
    pub async fn sign_out(&self, session_id: Option<String>) -> Result<(), String> {
        match session_id {
            Some(sid) => {
                self.api_client
                    .remove_session(&sid)
                    .await
                    .map_err(|e| format!("Failed to remove session: {}", e))?;
            }
            None => {
                self.api_client
                    .remove_client_sessions_and_retain_cookie()
                    .await
                    .map_err(|e| format!("Failed to remove all sessions: {}", e))?;
            }
        };
        // The remove sessions calls will update the client state via the callback

        Ok(())
    }

    /// Updates the active session and/or organization
    ///
    /// This method allows changing the active session and/or organization for the current client.
    /// It can be used to switch between different sessions or organizations that the user
    /// has access to. After the update, the client state will be refreshed via the callback.
    ///
    /// # Arguments
    /// * `session_id` - Optional session ID to set as active. If None, uses the current session.
    /// * `organization_id_or_slug` - Optional organization ID or slug to set as active. If None, no change to organization.
    ///
    /// # Returns
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    /// Returns an error if:
    /// - Client is not loaded
    /// - Both arguments are None
    /// - Session ID is not found in client sessions
    /// - Organization ID/slug is not found in user's memberships
    pub async fn set_active(
        &self,
        session_id: Option<String>,
        organization_id_or_slug: Option<String>,
    ) -> Result<(), String> {
        // Check if client is loaded
        if !self.loaded() {
            return Err("Cannot set active session before client is loaded".to_string());
        }

        // Get the client, target session, and organization information
        // while keeping the lock scope as small as possible
        let session_id_to_touch;
        let target_organization_id_option;

        {
            let mut state = self.state.write();
            let client = state.client.as_ref().ok_or("Client not found")?;

            // Get the target session either from the argument or current session
            let target_session = if let Some(sid) = session_id.clone() {
                client
                    .sessions
                    .iter()
                    .find(|s| s.id == sid)
                    .cloned()
                    .ok_or_else(|| format!("Session with ID {} not found", sid))?
            } else {
                state
                    .session
                    .clone()
                    .ok_or("No active session and no session_id provided")?
            };

            let user = match &target_session.user {
                Some(user_value) => *user_value.clone(),
                _ => return Err("No user data found in session".to_string()),
            };

            let target_organization_id = if let Some(org_id_or_slug) = organization_id_or_slug.clone() {
                if org_id_or_slug.starts_with("org_") {
                    // It's an organization ID - verify it exists in user's memberships
                    let org_exists = user
                        .organization_memberships
                        .as_ref()
                        .map(|memberships| {
                            memberships
                                .iter()
                                .any(|m| m.organization.id == org_id_or_slug)
                        })
                        .unwrap_or(false);
                    if !org_exists {
                        return Err(format!("Organization with ID {} not found in user's memberships", org_id_or_slug));
                    } else {
                        Some(org_id_or_slug)
                    }
                } else {
                    // Try to find organization by slug
                    let org_id = user
                        .organization_memberships
                        .as_ref()
                        .and_then(|memberships| {
                            memberships.iter().find_map(|m| {
                                if m.organization.slug == org_id_or_slug {
                                    Some(m.organization.id.clone())
                                } else {
                                    None
                                }
                            })
                        });
                    
                    // Return an error if organization is not found by slug
                    if org_id.is_none() {
                        return Err(format!("Organization with slug '{}' not found in user's memberships", org_id_or_slug));
                    }
                    
                    org_id
                }
            } else {
                None
            };

            // Save for API call
            session_id_to_touch = target_session.id;
            target_organization_id_option = target_organization_id.clone();

            // Update state
            state.target_organization_id = Some(target_organization_id);
        }

        // Now make the API call without holding any locks
        let active_organization_id = target_organization_id_option.as_deref();
        self.api_client
            .touch_session(&session_id_to_touch, active_organization_id)
            .await
            .map_err(|e| format!("Failed to touch session: {}", e))?;

        // We rely on the callback to update the state
        Ok(())
    }

    /// Updates the environment state with new environment data
    ///
    /// This method updates the internal state with new environment data and
    /// saves it to the store for persistence.
    ///
    /// # Arguments
    /// * `environment` - The new environment data to update state with
    ///
    /// # Returns
    /// Returns a Result containing () if successful
    ///
    /// # Errors
    /// Returns an error if serialization of environment data fails
    fn update_environment(&self, environment: Environment) -> Result<(), String> {
        // Update state
        {
            let mut state = self.state.write();
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
    ///
    /// Registers a callback function that will be notified of client state changes.
    /// The listener receives the current Client, Session, User and Organization state
    /// whenever it changes. If there's already a loaded client when the listener is added,
    /// the callback will be called immediately with the current state.
    ///
    /// # Arguments
    /// * `callback` - A function that takes the client, session, user, and organization as parameters
    pub fn add_listener<F>(&self, callback: F)
    where
        F: Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync + 'static,
    {
        let listener = Arc::new(callback);
        {
            let mut listeners = self.listeners.write();
            listeners.push(listener.clone());
        }

        // Then separately call the callback if we have a loaded client
        // Get state values with as small a lock scope as possible
        let maybe_client;
        let maybe_session;
        let maybe_user;
        let maybe_organization;

        {
            // Use try_read to avoid blocking if another lock is held
            let state = self.state.read();
            maybe_client = state.client.clone();
            maybe_session = state.session.clone();
            maybe_user = state.user.clone();
            maybe_organization = state.organization.clone();
        }

        // Call the callback if we have a client
        if let Some(client) = maybe_client {
            listener(client, maybe_session, maybe_user, maybe_organization);
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
                "cookie_expires_at": null,
                "captcha_bypass": false,
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
                          "claimed_at": 0,
                          "reverification": false,
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
        assert!(result.environment().is_some());
    }

    #[tokio::test]
    async fn test_init_environment_failure() {
        let mut server = Server::new_async().await;

        // Mock failed environment endpoint with /v1 prefix
        let env_mock = server
            .mock("GET", "/v1/environment?_is_native=1")
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
                          "claimed_at": 0,
                          "demo": false,
                          "reverification": false
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
                              "external_verification_redirect_url": null,
                              "attempts": null,
                              "expire_at": 0
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
                            "provider": "google",
                            "identification_id": "987654321012345678901",
                            "provider_user_id": "987654321012345678901",
                            "approved_scopes": "email https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile openid profile",
                            "email_address": "john.doe@example.com",
                            "first_name": "John",
                            "last_name": "Doe",
                            "image_url": "https://example.com/photos/abc123.jpg",
                            "username": "",
                            "public_metadata": {},
                            "label": null,
                            "created_at": 1717411902313i64,
                            "updated_at": 1730105981619i64,
                            "verification": {
                                "status": "verified",
                                "strategy": "from_oauth_google",
                                "external_verification_redirect_url": null,
                                "attempts": null,
                                "expire_at": 0
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
                      "factor_verification_age": [60],
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
                  "captcha_bypass": false,
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
        assert!(initialized_client.loaded());
        assert!(initialized_client.environment().is_some());
        assert!(initialized_client.client().is_some());
        assert!(initialized_client.session().is_some());
        assert!(initialized_client.user().is_some());
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
            let mut state = client.state.write();
            state.loaded = true;
            state.session = Some(Session {
                id: "sess_123".to_string(),
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
            let mut state = client.state.write();
            state.loaded = false;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);

        // Test with no session
        {
            let mut state = client.state.write();
            state.loaded = true;
            state.session = None;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);

        // Test with no user
        {
            let mut state = client.state.write();
            state.session = Some(Session {
                id: "sess_123".to_string(),
                ..Default::default()
            });
            state.user = None;
        }
        let token = client.get_token(None, None).await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn test_listener() {
        let config =
            ClerkFapiConfiguration::new("pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(), None, None)
                .unwrap();

        let clerk = Clerk::new(config);
        let was_called = Arc::new(AtomicBool::new(false));
        let was_called_clone = was_called.clone();

        // Add a listener
        clerk.add_listener(move |client, session, user, org| {
            assert_eq!(client.id, "test_client".to_string());
            assert!(session.is_some());
            assert!(user.is_some());
            assert!(org.is_none());
            was_called_clone.store(true, Ordering::SeqCst);
        });

        // Create test data
        let test_client = Client {
            id: "test_client".to_string(),
            sessions: vec![Session {
                id: "test_session".to_string(),
                user: Some(Box::new(User::default())),
                ..Default::default()
            }],
            last_active_session_id: Some("test_session".to_string()),
            ..Default::default()
        };

        // Update client which should trigger listener
        let clerk = clerk.clone();
        clerk.update_client(test_client).unwrap();

        // Verify listener was called
        assert!(was_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_listener_immediate_callback() {
        let config =
            ClerkFapiConfiguration::new("pk_test_Y2xlcmsuZXhhbXBsZS5jb20k".to_string(), None, None)
                .unwrap();

        let clerk = Clerk::new(config);

        // Set up initial state
        let test_client = Client {
            id: "test_client".to_string(),
            sessions: vec![Session {
                id: "test_session".to_string(),
                user: Some(Box::new(User::default())),
                ..Default::default()
            }],
            last_active_session_id: Some("test_session".to_string()),
            ..Default::default()
        };

        // Update client before adding listener
        let clerk = clerk.clone();
        clerk.update_client(test_client).unwrap();

        let was_called = Arc::new(AtomicBool::new(false));
        let was_called_clone = was_called.clone();

        // Add a listener - should be called immediately
        clerk.add_listener(move |client, session, user, org| {
            assert_eq!(client.id, "test_client".to_string());
            assert!(session.is_some());
            assert!(user.is_some());
            assert!(org.is_none());
            was_called_clone.store(true, Ordering::SeqCst);
        });

        // Verify listener was called immediately
        assert!(was_called.load(Ordering::SeqCst));
    }
}
