use crate::apis::configuration::Configuration as ApiConfiguration;
use crate::clerk_fapi::ClerkFapiClient;
use crate::configuration::{ClerkFapiConfiguration, ClientKind};
use crate::models::{
    ClientClientWrappedOrganizationMembershipsResponse, ClientPeriodClient as Client,
    ClientPeriodEnvironment as Environment, ClientPeriodOrganization as Organization,
    ClientPeriodOrganizationMembership, ClientPeriodSession as Session, ClientPeriodUser as User,
};
use futures::TryFutureExt;
use log::warn;
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

        if self.config.is_development() && self.config.kind == ClientKind::Browser {
            let dev_browser = self
                .api_client
                .create_dev_browser()
                .await
                .map_err(|e| e.to_string())?;
            self.api_client.set_dev_browser_token_id(dev_browser.id);
        }

        self.load_environment().await?;
        self.load_client().await?;

        // Set loaded flag
        {
            let mut state = self.state.write();
            state.loaded = true;
        }

        if self.config.is_development() {
            warn!("Clerk: Clerk has been loaded with development keys. Development instances have strict usage limits and should not be used when deploying your application to production. Learn more: https://clerk.com/docs/deployments/overview")
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

        let target_session = {
            let state = self.state.read();
            let client = state.client.as_ref().ok_or("Client not found")?;
            // Get the target session either from the argument or current session
            if let Some(sid) = session_id.clone() {
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
            }
        };
        let session_id_to_touch = target_session.id;

        let mut target_organization_id_option = None::<String>;

        if let Some(organization_id_or_slug) = organization_id_or_slug {
            let user = match &target_session.user {
                Some(user_value) => *user_value.clone(),
                _ => return Err("No user data found in session".to_string()),
            };

            if let Some(user_org_memberships) = user.organization_memberships {
                target_organization_id_option = find_organization_id_from_memberships(
                    user_org_memberships,
                    organization_id_or_slug.clone(),
                )
                .map(|m| m.organization.id);
            }

            if target_organization_id_option.is_none() {
                // we couldn't find the org, perhaps the org is new
                // and we just don't have it yet so let's check from the api!
                let user = *self
                    .api_client
                    .get_user(Some(&session_id_to_touch))
                    .await
                    .map_err(|e| e.to_string())?
                    .response;
                if let Some(user_org_memberships) = user.organization_memberships {
                    target_organization_id_option = find_organization_id_from_memberships(
                        user_org_memberships,
                        organization_id_or_slug.clone(),
                    )
                    .map(|m| m.organization.id);
                }
            }

            if target_organization_id_option.is_none() {
                // still could not find! Let's try to forcefully pull all the orgs
                let org_memberships = *self
                    .api_client
                    .get_organization_memberships(
                        None, // limit
                        None, // offset
                        None, // paginated
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .response;

                let user_org_memberships = match org_memberships {
                    ClientClientWrappedOrganizationMembershipsResponse::Array(memberships) => {
                        memberships
                    },
                    ClientClientWrappedOrganizationMembershipsResponse::ClientClientWrappedOrganizationMembershipsResponseOneOf(memberships) => {
                        memberships.data.unwrap()
                    }
                };
                target_organization_id_option = find_organization_id_from_memberships(
                    user_org_memberships,
                    organization_id_or_slug.clone(),
                )
                .map(|m| m.organization.id);
            }

            if target_organization_id_option.is_none() {
                // if we still couldn't find the org we need to error out
                return Err("Could not find organization".into());
            }
        }

        {
            let mut state = self.state.write();
            state.target_organization_id = Some(target_organization_id_option.clone());
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

fn find_organization_id_from_memberships(
    memberships: Vec<ClientPeriodOrganizationMembership>,
    organization_id_or_slug: String,
) -> Option<ClientPeriodOrganizationMembership> {
    if organization_id_or_slug.starts_with("org_") {
        // It's an organization ID - verify it exists in memberships
        memberships
            .into_iter()
            .find(|m| m.organization.id == organization_id_or_slug)
    } else {
        // It's a slug
        memberships
            .into_iter()
            .find(|m| m.organization.slug == organization_id_or_slug)
    }
}

