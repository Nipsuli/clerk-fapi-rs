use crate::apis::configuration::Configuration as ApiConfiguration;
use crate::clerk_fapi::ClerkFapiClient;
use crate::clerk_state::{ClerkNotLoadedError, ClerkState};
use crate::configuration::{ClerkFapiConfiguration, ClientKind};
use crate::models::{
    ClientClientWrappedOrganizationMembershipsResponse, ClientPeriodClient as Client,
    ClientPeriodEnvironment as Environment, ClientPeriodOrganization as Organization,
    ClientPeriodOrganizationMembership, ClientPeriodSession as Session, ClientPeriodUser as User,
};
use crate::utils::{
    find_organization_id_from_memberships, find_target_organization, find_target_session,
    ClerkOrgFindingError, ClerkSessionFindingError,
};
use futures::TryFutureExt;
use log::{error, warn};
use parking_lot::{RwLock, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub type Listener =
    Arc<dyn Fn(Client, Option<Session>, Option<User>, Option<Organization>) + Send + Sync>;

/// The main client for interacting with Clerk's Frontend API
#[derive(Clone)]
pub struct Clerk {
    config: Arc<ClerkFapiConfiguration>,
    state: Arc<RwLock<ClerkState>>,
    api_client: Arc<ClerkFapiClient>,
    listeners: Arc<RwLock<Vec<Listener>>>,
}

#[derive(Debug)]
pub enum ClerkLoadError {
    FailedToLoadEnv,
    FailedToLoadClient,
}
impl fmt::Display for ClerkLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClerkLoadError::FailedToLoadEnv => write!(f, "Failed to load Clerk environment"),
            ClerkLoadError::FailedToLoadClient => write!(f, "Failed to load Clerk client"),
        }
    }
}
impl Error for ClerkLoadError {}

#[derive(Debug)]
pub enum ClerkSetActiveError {
    ClerkNotLoadedError(ClerkNotLoadedError),
    ClerkOrgFindingError(ClerkOrgFindingError),
    ClerkSessionFindingError(ClerkSessionFindingError),
    ClerkApiError,
}
impl fmt::Display for ClerkSetActiveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClerkSetActiveError::ClerkNotLoadedError(e) => e.fmt(f),
            ClerkSetActiveError::ClerkOrgFindingError(e) => e.fmt(f),
            ClerkSetActiveError::ClerkSessionFindingError(e) => e.fmt(f),
            ClerkSetActiveError::ClerkApiError => write!(f, "Error calling Clerk API"),
        }
    }
}
impl Error for ClerkSetActiveError {}

#[derive(Debug)]
pub enum ClerkGetTokenError {
    ClerkNotLoadedError(ClerkNotLoadedError),
    ClerkApiError,
}
impl fmt::Display for ClerkGetTokenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClerkGetTokenError::ClerkNotLoadedError(e) => e.fmt(f),
            ClerkGetTokenError::ClerkApiError => write!(f, "Error in Clerk API"),
        }
    }
}
impl Error for ClerkGetTokenError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClerkLoadResult {
    pub environment_loaded_from_cache: bool,
    pub client_loaded_from_cache: bool,
}

impl Clerk {
    /// Creates a new Clerk client with the provided configuration
    ///
    /// This constructor initializes a new client with the given configuration,
    /// setting up the necessary internal state and API client for interacting
    /// with Clerk's Frontend API.
    pub fn new(config: ClerkFapiConfiguration) -> Self {
        let listeners = Arc::new(RwLock::new(Vec::<Listener>::new()));

        let listeners_inner = listeners.clone();
        let state = Arc::new(RwLock::new(ClerkState::new(
            config.clone(),
            move |client, session, user, organization| {
                let cbs = { listeners_inner.read() };
                for cb in cbs.iter() {
                    cb(
                        client.clone(),
                        session.clone(),
                        user.clone(),
                        organization.clone(),
                    );
                }
            },
        )));

        let api_client = Arc::new(ClerkFapiClient::new(config.clone(), state.clone()).unwrap());

        Self {
            config: Arc::new(config),
            state,
            api_client,
            listeners,
        }
    }

    //
    // Load Helpers
    //

    fn load_environment_from_cache(&self) -> Option<Environment> {
        if let Some(stored_env) = self.config.get_store_value("environment") {
            if let Ok(cached_environment) = serde_json::from_value::<Environment>(stored_env) {
                Some(cached_environment)
            } else {
                error!("Failed to deserialize environment from cache");
                None
            }
        } else {
            None
        }
    }

    async fn load_environment_from_api(&self) -> Result<Environment, ClerkLoadError> {
        self.api_client.get_environment().await.map_err(|e| {
            error!("Clerk: Failed to load environment from API: {e}");
            ClerkLoadError::FailedToLoadEnv
        })
    }

    fn load_client_from_cache(&self) -> Option<Client> {
        if let Some(stored_client) = self.config.get_store_value("client") {
            if let Ok(cached_client) = serde_json::from_value::<Client>(stored_client) {
                Some(cached_client)
            } else {
                error!("Failed to deserialize client from cache");
                None
            }
        } else {
            None
        }
    }

    async fn load_client_from_api(&self) -> Result<Client, ClerkLoadError> {
        let client_res = self
            .api_client
            .get_client()
            .await
            .map_err(|e| {
                error!("Clerk: Failed to load client from API: {e}");
                ClerkLoadError::FailedToLoadClient
            })?
            .response
            .ok_or(ClerkLoadError::FailedToLoadClient)?;
        Ok(*client_res)
    }

    /// Initializes Clerk, one can optionally load the Environment and Client
    /// From the cache. Example if one uses persisted ClerkStore one can pass
    /// prefer_cache: True to prefer loading from the persisted state to load
    /// Clerk without needing to do network calls in case where one has loaded
    /// the client already once before.
    pub async fn load(&self, prefer_cache: bool) -> Result<ClerkLoadResult, ClerkLoadError> {
        let mut environment = None;
        let mut client = None;

        let mut res = ClerkLoadResult {
            environment_loaded_from_cache: false,
            client_loaded_from_cache: false,
        };

        if prefer_cache {
            environment = self.load_environment_from_cache();
            if environment.is_some() {
                res.environment_loaded_from_cache = true;
            }
            client = self.load_client_from_cache();
            if client.is_some() {
                res.client_loaded_from_cache = true;
            }
        }

        let environment = match environment {
            Some(e) => e,
            None => self.load_environment_from_api().await?,
        };
        let client = match client {
            Some(c) => c,
            None => self.load_client_from_api().await?,
        };
        self.set_loaded(environment, client);

        Ok(res)
    }

    /// set_loaded is public method, example in scenario where we endup
    /// loading the Environement and Client else where, we can initialize
    /// the Clerk with already existing environment and client
    pub fn set_loaded(&self, environment: Environment, client: Client) {
        {
            let mut state = self.state.write();
            state.set_loaded(environment, client);
        }
        // After loading we trigger possible listeners that were
        // set before Clerk was loaded
        let state = self.state.read();
        state.emit_state();
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

        let client;
        let session;
        let user;
        let organization;

        {
            let state = self.state.read();
            client = state.client();
            session = state.session();
            user = state.user();
            organization = state.organization();
        }

        match (client, session, user, organization) {
            (Ok(client), Ok(session), Ok(user), Ok(organization)) => {
                listener(client, session, user, organization);
            }
            _ => {
                warn!("Clerk: added listener before Clerk was loaded");
            }
        }
    }

    //
    // To be able to use Clerk example in Tauri app where one needs to
    // hook to the fapi request hooks in js side we expose the client
    // authorization header getter and setter
    //

    pub fn get_client_authorization_header(&self) -> Result<Option<String>, ClerkNotLoadedError> {
        self.state.write().authorization_header()
    }
    pub fn set_client_authorization_header(
        &self,
        header: Option<String>,
    ) -> Result<(), ClerkNotLoadedError> {
        if !self.loaded() {
            Err(ClerkNotLoadedError::NotLoaded)
        } else {
            Ok(self.state.write().set_authorization_header(header))
        }
    }

    //
    // Data access methods
    //

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
    pub fn environment(&self) -> Result<Environment, ClerkNotLoadedError> {
        self.state.read().environment()
    }

    /// Returns the current client data if initialized
    ///
    /// Provides access to the Clerk client data, which includes information about
    /// the current browser/device client and its associated sessions.
    /// Returns None if the client hasn't been loaded yet.
    pub fn client(&self) -> Result<Client, ClerkNotLoadedError> {
        self.state.read().client()
    }

    /// Returns the current active session if available
    ///
    /// Provides access to the user's active session, which contains authentication
    /// state and session-specific data. Returns None if no active session exists
    /// or if the client hasn't been loaded yet.
    pub fn session(&self) -> Result<Option<Session>, ClerkNotLoadedError> {
        self.state.read().session()
    }

    /// Returns the current authenticated user if available
    ///
    /// Provides access to the authenticated user associated with the active session.
    /// Returns user data including profile information, email addresses, and organization memberships.
    /// Returns None if no user is authenticated or if the client hasn't been loaded yet.
    pub fn user(&self) -> Result<Option<User>, ClerkNotLoadedError> {
        self.state.read().user()
    }

    /// Returns the active organization if available
    ///
    /// Provides access to the currently active organization for the authenticated user.
    /// This is the organization that was last activated in the session, or was specified
    /// with `set_active()`. Returns None if no organization is active or if the client
    /// hasn't been loaded yet.
    pub fn organization(&self) -> Result<Option<Organization>, ClerkNotLoadedError> {
        self.state.read().organization()
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
    ) -> Result<Option<String>, ClerkGetTokenError> {
        let session = match self
            .session()
            .map_err(|e| ClerkGetTokenError::ClerkNotLoadedError(e))?
        {
            Some(s) => s,
            None => {
                // No active session -> cannot return token
                return Ok(None);
            }
        };

        if self
            .user()
            .map_err(|e| ClerkGetTokenError::ClerkNotLoadedError(e))?
            .is_none()
        {
            // session but no user
            return Ok(None);
        }

        // Call appropriate token creation method based on parameters
        let result = match template {
            Some(template_name) => self
                .api_client
                .create_session_token_with_template(&session.id, template_name)
                .await
                .map_err(|e| {
                    error!("Failed to call create_session_token_with_template: {e}");
                    ClerkGetTokenError::ClerkApiError
                })?,
            None => self
                .api_client
                .create_session_token(&session.id, organization_id)
                .await
                .map_err(|e| {
                    error!("Failed to call create_session_token: {e}");
                    ClerkGetTokenError::ClerkApiError
                })?,
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
    ) -> Result<(), ClerkSetActiveError> {
        let target_session = {
            let state = self.state.read();
            let client = state
                .client()
                .map_err(|e| ClerkSetActiveError::ClerkNotLoadedError(e))?;
            if let Some(session_id) = session_id.clone() {
                find_target_session(client.clone(), session_id)
                    .map_err(|e| ClerkSetActiveError::ClerkSessionFindingError(e))?
            } else {
                state
                    .session()
                    .map_err(|e| ClerkSetActiveError::ClerkNotLoadedError(e))?
                    .ok_or(ClerkSetActiveError::ClerkSessionFindingError(
                        ClerkSessionFindingError::NoSession,
                    ))?
            }
        };
        let session_id_to_touch = target_session.clone().id;

        let target_organization_id_option = match organization_id_or_slug {
            None => None::<String>,
            Some(organization_id_or_slug) => {
                let target_organization_id = find_target_organization(
                    self.get_fapi_client(),
                    target_session.clone(),
                    organization_id_or_slug,
                )
                .await
                .map(|o| o.id)
                .map_err(|e| ClerkSetActiveError::ClerkOrgFindingError(e))?;
                {
                    // We found target org, we need to store the target org
                    // to the state, so that after the session touching we know
                    // to unpack correct org
                    let mut state = self.state.write();
                    state.set_target_orgnization(Some(target_organization_id.clone()));
                }
                Some(target_organization_id)
            }
        };

        let active_organization_id = target_organization_id_option.as_deref();
        // Touch session to activate it
        self.api_client
            .touch_session(&session_id_to_touch, active_organization_id)
            .await
            .map_err(|e| {
                error!("Failed to touch session: {}", e);
                ClerkSetActiveError::ClerkApiError
            })?;

        // We rely on the callback mechanism to update the state
        Ok(())
    }
}
