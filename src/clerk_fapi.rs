use crate::apis::configuration::Configuration as ApiConfiguration;
use crate::apis::*;
use crate::configuration::{ClerkFapiConfiguration, Store};
use crate::models::*;
use async_trait::async_trait;
use http::Extensions as HttpExtensions;
use parking_lot::Mutex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use reqwest::{Request, Response};
use reqwest_middleware::{
    ClientBuilder, ClientWithMiddleware, Middleware, Next, Result as ReqwestResult,
};
use serde_json::Value as JsonValue;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Add middleware definitions
#[derive(Clone)]
struct DefaultQueryMiddleware;

#[async_trait]
impl Middleware for DefaultQueryMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut HttpExtensions,
        next: Next<'_>,
    ) -> ReqwestResult<Response> {
        let url = req.url_mut();
        url.query_pairs_mut().append_pair("_is_native", "1");
        next.run(req, extensions).await
    }
}

#[derive(Clone)]
struct AuthorizationMiddleware {
    store: Arc<dyn Store>,
    store_prefix: String,
}

impl AuthorizationMiddleware {
    fn new(store: Arc<dyn Store>, store_prefix: String) -> Self {
        Self {
            store,
            store_prefix,
        }
    }

    fn get_auth_key(&self) -> String {
        format!("{}authorization", self.store_prefix)
    }
}

#[async_trait]
impl Middleware for AuthorizationMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut HttpExtensions,
        next: Next<'_>,
    ) -> ReqwestResult<Response> {
        if let Some(auth) = self.store.get(&self.get_auth_key()) {
            if let Some(auth_str) = auth.as_str() {
                if let Ok(value) = HeaderValue::from_str(auth_str) {
                    req.headers_mut().insert("Authorization", value);
                }
            }
        }

        let store = self.store.clone();
        let auth_key = self.get_auth_key();

        let resp = next.run(req, extensions).await?;

        if let Some(auth_header) = resp.headers().get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                store.set(&auth_key, JsonValue::String(auth_str.to_string()));
            }
        }

        Ok(resp)
    }
}

/// Type definition for the client update callback function
type ClientUpdateCallback = Box<dyn FnMut(client_period_client::ClientPeriodClient) + Send>;

/// The main client for interacting with Clerk's Frontend API
#[derive(Clone)]
pub struct ClerkFapiClient {
    config: Arc<ApiConfiguration>,
    update_client_callback: Option<Arc<Mutex<ClientUpdateCallback>>>,
}

impl ClerkFapiClient {
    /// Creates a new ClerkFapiClient with the provided configuration
    pub fn new(config: ClerkFapiConfiguration) -> Result<Self, String> {
        // Create default headers
        let mut headers = HeaderMap::new();
        headers.insert("x-mobile", HeaderValue::from_static("1"));
        headers.insert("x-no-origin", HeaderValue::from_static("1"));

        // Create client with default headers and middleware
        let http_client = Client::builder()
            .default_headers(headers)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let client = ClientBuilder::new(http_client)
            .with(DefaultQueryMiddleware)
            .with(AuthorizationMiddleware::new(
                config.store.clone(),
                config.store_prefix.clone(),
            ))
            .build();

        // Create API configuration
        let mut api_config = ApiConfiguration::new();
        api_config.base_path = config.base_url.clone();
        api_config.user_agent = Some(config.user_agent.clone());
        api_config.client = client.clone();

        Ok(Self {
            config: Arc::new(api_config),
            update_client_callback: None,
        })
    }

    /// Sets the callback for client updates
    pub fn set_update_client_callback<F>(&mut self, callback: F)
    where
        F: FnMut(client_period_client::ClientPeriodClient) + Send + 'static,
    {
        self.update_client_callback = Some(Arc::new(Mutex::new(Box::new(callback))));
    }

    fn handle_client_update(
        &self,
        client: client_period_client::ClientPeriodClient,
    ) -> Result<(), String> {
        if let Some(cb) = &self.update_client_callback {
            let mut cb = cb.lock(); // Lock the Mutex to get mutable access
            (cb)(client); // Call the synchronous callback
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Returns a reference to the client's API configuration
    pub fn config(&self) -> &ApiConfiguration {
        &self.config
    }

    // Active Sessions API methods
    pub async fn get_sessions(
        &self,
        clerk_session_id: Option<&str>,
    ) -> Result<Vec<ClientPeriodActiveSession>, Error<active_sessions_api::GetSessionsError>> {
        active_sessions_api::get_sessions(&self.config, clerk_session_id).await
    }

    pub async fn get_users_sessions(
        &self,
        clerk_session_id: Option<&str>,
    ) -> Result<Vec<ClientPeriodSession>, Error<active_sessions_api::GetUsersSessionsError>> {
        active_sessions_api::get_users_sessions(&self.config, clerk_session_id).await
    }

    pub async fn revoke_session(
        &self,
        session_id: &str,
        clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSession, Error<active_sessions_api::RevokeSessionError>>
    {
        let response =
            active_sessions_api::revoke_session(&self.config, session_id, clerk_session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    // Backup Codes API methods
    pub async fn create_backup_codes(
        &self,
    ) -> Result<ClientPeriodClientWrappedBackupCodes, Error<backup_codes_api::CreateBackupCodesError>>
    {
        let response = backup_codes_api::create_backup_codes(&self.config).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Client API methods
    pub async fn delete_client_sessions(
        &self,
    ) -> Result<ClientPeriodDeleteSession, Error<client_api::DeleteClientSessionsError>> {
        let response = client_api::delete_client_sessions(&self.config).await?;
        if let Some(client) = response.response.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn get_client(
        &self,
    ) -> Result<ClientPeriodClientWrappedClient, Error<client_api::GetClientError>> {
        client_api::get_client(&self.config).await
    }

    pub async fn handshake_client(
        &self,
        clerk_proxy_url: Option<&str>,
        clerk_secret_key: Option<&str>,
        redirect_url: Option<&str>,
        organization_id: Option<&str>,
        satellite_fapi: Option<&str>,
    ) -> Result<(), Error<client_api::HandshakeClientError>> {
        client_api::handshake_client(
            &self.config,
            clerk_proxy_url,
            clerk_secret_key,
            redirect_url,
            organization_id,
            satellite_fapi,
        )
        .await
    }

    pub async fn post_client(
        &self,
    ) -> Result<ClientPeriodClientWrappedClient, Error<client_api::PostClientError>> {
        client_api::post_client(&self.config).await
    }

    pub async fn put_client(
        &self,
    ) -> Result<ClientPeriodClientWrappedClient, Error<client_api::PutClientError>> {
        client_api::put_client(&self.config).await
    }

    // Default API methods
    pub async fn clear_site_data(&self) -> Result<(), Error<default_api::ClearSiteDataError>> {
        default_api::clear_site_data(&self.config).await
    }

    pub async fn get_account_portal(
        &self,
    ) -> Result<ClientPeriodAccountPortal, Error<default_api::GetAccountPortalError>> {
        default_api::get_account_portal(&self.config).await
    }

    pub async fn get_dev_browser_init(
        &self,
        origin: Option<&str>,
    ) -> Result<(), Error<default_api::GetDevBrowserInitError>> {
        default_api::get_dev_browser_init(&self.config, origin).await
    }

    pub async fn get_proxy_health(
        &self,
        domain_id: &str,
        clerk_proxy_url: &str,
        clerk_secret_key: &str,
        x_forwarded_for: &str,
    ) -> Result<GetProxyHealth200Response, Error<default_api::GetProxyHealthError>> {
        default_api::get_proxy_health(
            &self.config,
            domain_id,
            clerk_proxy_url,
            clerk_secret_key,
            x_forwarded_for,
        )
        .await
    }

    pub async fn link_client(
        &self,
        clerk_token: Option<&str>,
    ) -> Result<(), Error<default_api::LinkClientError>> {
        default_api::link_client(&self.config, clerk_token).await
    }

    pub async fn post_dev_browser_init_set_cookie(
        &self,
    ) -> Result<(), Error<default_api::PostDevBrowserInitSetCookieError>> {
        default_api::post_dev_browser_init_set_cookie(&self.config).await
    }

    pub async fn sync_client(
        &self,
        link_domain: Option<&str>,
        redirect_url: Option<&str>,
    ) -> Result<(), Error<default_api::SyncClientError>> {
        default_api::sync_client(&self.config, link_domain, redirect_url).await
    }

    // Dev Browser API methods
    pub async fn create_dev_browser(
        &self,
    ) -> Result<(), Error<dev_browser_api::CreateDevBrowserError>> {
        dev_browser_api::create_dev_browser(&self.config).await
    }

    // Domains API methods
    pub async fn attempt_organization_domain_verification(
        &self,
        organization_id: &str,
        domain_id: &str,
        code: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomain,
        Error<domains_api::AttemptOrganizationDomainVerificationError>,
    > {
        let response = domains_api::attempt_organization_domain_verification(
            &self.config,
            organization_id,
            domain_id,
            code,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn create_organization_domain(
        &self,
        organization_id: &str,
        name: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomain,
        Error<domains_api::CreateOrganizationDomainError>,
    > {
        let response =
            domains_api::create_organization_domain(&self.config, organization_id, name).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_organization_domain(
        &self,
        organization_id: &str,
        domain_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<domains_api::DeleteOrganizationDomainError>,
    > {
        let response =
            domains_api::delete_organization_domain(&self.config, organization_id, domain_id)
                .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_organization_domain(
        &self,
        organization_id: &str,
        domain_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomain,
        Error<domains_api::GetOrganizationDomainError>,
    > {
        let response =
            domains_api::get_organization_domain(&self.config, organization_id, domain_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn list_organization_domains(
        &self,
        organization_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        verified: Option<bool>,
        enrollment_mode: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomains,
        Error<domains_api::ListOrganizationDomainsError>,
    > {
        let response = domains_api::list_organization_domains(
            &self.config,
            organization_id,
            limit,
            offset,
            verified,
            enrollment_mode,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn prepare_organization_domain_verification(
        &self,
        organization_id: &str,
        domain_id: &str,
        affiliation_email_address: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomain,
        Error<domains_api::PrepareOrganizationDomainVerificationError>,
    > {
        let response = domains_api::prepare_organization_domain_verification(
            &self.config,
            organization_id,
            domain_id,
            affiliation_email_address,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn update_organization_domain_enrollment_mode(
        &self,
        organization_id: &str,
        domain_id: &str,
        enrollment_mode: &str,
        delete_pending: Option<bool>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationDomain,
        Error<domains_api::UpdateOrganizationDomainEnrollmentModeError>,
    > {
        let response = domains_api::update_organization_domain_enrollment_mode(
            &self.config,
            organization_id,
            domain_id,
            enrollment_mode,
            delete_pending,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Email Addresses API methods
    pub async fn create_email_addresses(
        &self,
        email_address: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedEmailAddress,
        Error<email_addresses_api::CreateEmailAddressesError>,
    > {
        let response = email_addresses_api::create_email_addresses(
            &self.config,
            email_address,
            _clerk_session_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_email_address(
        &self,
        email_id: &str,
        clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<email_addresses_api::DeleteEmailAddressError>,
    > {
        let response =
            email_addresses_api::delete_email_address(&self.config, email_id, clerk_session_id)
                .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_email_address(
        &self,
        email_id: &str,
        clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedEmailAddress,
        Error<email_addresses_api::GetEmailAddressError>,
    > {
        let response =
            email_addresses_api::get_email_address(&self.config, email_id, clerk_session_id)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn get_email_addresses(
        &self,
        clerk_session_id: Option<&str>,
    ) -> Result<Vec<ClientPeriodEmailAddress>, Error<email_addresses_api::GetEmailAddressesError>>
    {
        email_addresses_api::get_email_addresses(&self.config, clerk_session_id).await
    }

    pub async fn send_verification_email(
        &self,
        email_id: &str,
        strategy: &str,
        _clerk_session_id: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedEmailAddress,
        Error<email_addresses_api::SendVerificationEmailError>,
    > {
        let response = email_addresses_api::send_verification_email(
            &self.config,
            email_id,
            strategy,
            _clerk_session_id,
            redirect_url,
            action_complete_redirect_url,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn verify_email_address(
        &self,
        email_id: &str,
        code: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedEmailAddress,
        Error<email_addresses_api::VerifyEmailAddressError>,
    > {
        let response = email_addresses_api::verify_email_address(
            &self.config,
            email_id,
            code,
            _clerk_session_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Environment API methods
    pub async fn get_environment(
        &self,
    ) -> Result<ClientPeriodEnvironment, Error<environment_api::GetEnvironmentError>> {
        environment_api::get_environment(&self.config).await
    }

    pub async fn update_environment(
        &self,
        origin: &str,
    ) -> Result<ClientPeriodEnvironment, Error<environment_api::UpdateEnvironmentError>> {
        environment_api::update_environment(&self.config, origin).await
    }

    // External Accounts API methods
    pub async fn delete_external_account(
        &self,
        external_account_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<external_accounts_api::DeleteExternalAccountError>,
    > {
        let response =
            external_accounts_api::delete_external_account(&self.config, external_account_id)
                .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn post_o_auth_accounts(
        &self,
        strategy: &str,
        origin: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        additional_scope: Option<&str>,
        code: Option<&str>,
        token: Option<&str>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedExternalAccount,
        Error<external_accounts_api::PostOAuthAccountsError>,
    > {
        let response = external_accounts_api::post_o_auth_accounts(
            &self.config,
            strategy,
            origin,
            redirect_url,
            action_complete_redirect_url,
            additional_scope,
            code,
            token,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn reauthorize_external_account(
        &self,
        external_account_id: &str,
        redirect_url: &str,
        additional_scope: Option<Vec<String>>,
        action_complete_redirect_url: Option<&str>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedExternalAccount,
        Error<external_accounts_api::ReauthorizeExternalAccountError>,
    > {
        let response = external_accounts_api::reauthorize_external_account(
            &self.config,
            external_account_id,
            redirect_url,
            additional_scope,
            action_complete_redirect_url,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn revoke_external_account_tokens(
        &self,
        external_account_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedUser,
        Error<external_accounts_api::RevokeExternalAccountTokensError>,
    > {
        let response = external_accounts_api::revoke_external_account_tokens(
            &self.config,
            external_account_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Health API methods
    pub async fn get_health(
        &self,
    ) -> Result<GetHealth200Response, Error<health_api::GetHealthError>> {
        health_api::get_health(&self.config).await
    }

    // Invitations API methods
    pub async fn bulk_create_organization_invitations(
        &self,
        organization_id: &str,
        email_address: Vec<String>,
        role: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitations,
        Error<invitations_api::BulkCreateOrganizationInvitationsError>,
    > {
        let response = invitations_api::bulk_create_organization_invitations(
            &self.config,
            organization_id,
            email_address,
            role,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn create_organization_invitations(
        &self,
        organization_id: &str,
        email_address: &str,
        role: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitation,
        Error<invitations_api::CreateOrganizationInvitationsError>,
    > {
        let response = invitations_api::create_organization_invitations(
            &self.config,
            organization_id,
            email_address,
            role,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn get_all_pending_organization_invitations(
        &self,
        organization_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitations,
        Error<invitations_api::GetAllPendingOrganizationInvitationsError>,
    > {
        let response = invitations_api::get_all_pending_organization_invitations(
            &self.config,
            organization_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn get_organization_invitations(
        &self,
        organization_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        status: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitations,
        Error<invitations_api::GetOrganizationInvitationsError>,
    > {
        let response = invitations_api::get_organization_invitations(
            &self.config,
            organization_id,
            limit,
            offset,
            status,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn revoke_pending_organization_invitation(
        &self,
        organization_id: &str,
        invitation_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitation,
        Error<invitations_api::RevokePendingOrganizationInvitationError>,
    > {
        let response = invitations_api::revoke_pending_organization_invitation(
            &self.config,
            organization_id,
            invitation_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Members API methods
    pub async fn create_organization_membership(
        &self,
        organization_id: &str,
        user_id: Option<&str>,
        role: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembership,
        Error<members_api::CreateOrganizationMembershipError>,
    > {
        let response = members_api::create_organization_membership(
            &self.config,
            organization_id,
            user_id,
            role,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn list_organization_memberships(
        &self,
        organization_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        paginated: Option<bool>,
        query: Option<&str>,
        role: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMemberships,
        Error<members_api::ListOrganizationMembershipsError>,
    > {
        let response = members_api::list_organization_memberships(
            &self.config,
            organization_id,
            limit,
            offset,
            paginated,
            query,
            role,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn remove_organization_member(
        &self,
        organization_id: &str,
        user_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembership,
        Error<members_api::RemoveOrganizationMemberError>,
    > {
        let response =
            members_api::remove_organization_member(&self.config, organization_id, user_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn update_organization_membership(
        &self,
        organization_id: &str,
        user_id: &str,
        role: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembership,
        Error<members_api::UpdateOrganizationMembershipError>,
    > {
        let response = members_api::update_organization_membership(
            &self.config,
            organization_id,
            user_id,
            role,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Membership Requests API methods
    pub async fn accept_organization_membership_request(
        &self,
        organization_id: &str,
        request_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembershipRequest,
        Error<membership_requests_api::AcceptOrganizationMembershipRequestError>,
    > {
        let response = membership_requests_api::accept_organization_membership_request(
            &self.config,
            organization_id,
            request_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn list_organization_membership_requests(
        &self,
        organization_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        status: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembershipRequests,
        Error<membership_requests_api::ListOrganizationMembershipRequestsError>,
    > {
        let response = membership_requests_api::list_organization_membership_requests(
            &self.config,
            organization_id,
            limit,
            offset,
            status,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn reject_organization_membership_request(
        &self,
        organization_id: &str,
        request_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMembershipRequest,
        Error<membership_requests_api::RejectOrganizationMembershipRequestError>,
    > {
        let response = membership_requests_api::reject_organization_membership_request(
            &self.config,
            organization_id,
            request_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // OAuth2 Callbacks API methods
    pub async fn get_oauth_callback(
        &self,
        scope: Option<&str>,
        code: Option<&str>,
        state: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), Error<o_auth2_callbacks_api::GetOauthCallbackError>> {
        o_auth2_callbacks_api::get_oauth_callback(&self.config, scope, code, state, error).await
    }

    pub async fn post_oauth_callback(
        &self,
        code: Option<&str>,
        scope: Option<&str>,
        state: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), Error<o_auth2_callbacks_api::PostOauthCallbackError>> {
        o_auth2_callbacks_api::post_oauth_callback(&self.config, code, scope, state, error).await
    }

    // OAuth2 Identity Provider API methods
    pub async fn get_o_auth_token(
        &self,
    ) -> Result<OAuthPeriodToken, Error<o_auth2_identify_provider_api::GetOAuthTokenError>> {
        o_auth2_identify_provider_api::get_o_auth_token(&self.config).await
    }

    pub async fn get_o_auth_user_info(
        &self,
    ) -> Result<OAuthPeriodUserInfo, Error<o_auth2_identify_provider_api::GetOAuthUserInfoError>>
    {
        o_auth2_identify_provider_api::get_o_auth_user_info(&self.config).await
    }

    pub async fn request_o_auth_authorize(
        &self,
    ) -> Result<(), Error<o_auth2_identify_provider_api::RequestOAuthAuthorizeError>> {
        o_auth2_identify_provider_api::request_o_auth_authorize(&self.config).await
    }

    // Organization API methods
    pub async fn create_organization(
        &self,
        name: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganization,
        Error<organization_api::CreateOrganizationError>,
    > {
        let response = organization_api::create_organization(&self.config, name).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_organization(
        &self,
        organization_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<organization_api::DeleteOrganizationError>,
    > {
        let response = organization_api::delete_organization(&self.config, organization_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn delete_organization_logo(
        &self,
        organization_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<organization_api::DeleteOrganizationLogoError>,
    > {
        let response =
            organization_api::delete_organization_logo(&self.config, organization_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_organization(
        &self,
        organization_id: &str,
    ) -> Result<ClientPeriodClientWrappedOrganization, Error<organization_api::GetOrganizationError>>
    {
        let response = organization_api::get_organization(&self.config, organization_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn update_organization(
        &self,
        organization_id: &str,
        name: Option<&str>,
        slug: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganization,
        Error<organization_api::UpdateOrganizationError>,
    > {
        let response =
            organization_api::update_organization(&self.config, organization_id, name, slug)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn update_organization_logo(
        &self,
        organization_id: &str,
        file: Option<std::path::PathBuf>,
    ) -> Result<
        ClientPeriodClientWrappedOrganization,
        Error<organization_api::UpdateOrganizationLogoError>,
    > {
        let response =
            organization_api::update_organization_logo(&self.config, organization_id, file).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Organization Memberships API methods
    pub async fn accept_organization_invitation(
        &self,
        invitation_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitationUserContext,
        Error<organizations_memberships_api::AcceptOrganizationInvitationError>,
    > {
        let response = organizations_memberships_api::accept_organization_invitation(
            &self.config,
            invitation_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn accept_organization_suggestion(
        &self,
        suggestion_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationSuggestion,
        Error<organizations_memberships_api::AcceptOrganizationSuggestionError>,
    > {
        let response = organizations_memberships_api::accept_organization_suggestion(
            &self.config,
            suggestion_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_organization_memberships(
        &self,
        organization_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<organizations_memberships_api::DeleteOrganizationMembershipsError>,
    > {
        let response = organizations_memberships_api::delete_organization_memberships(
            &self.config,
            organization_id,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_organization_memberships(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        paginated: Option<bool>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationMemberships,
        Error<organizations_memberships_api::GetOrganizationMembershipsError>,
    > {
        let response = organizations_memberships_api::get_organization_memberships(
            &self.config,
            limit,
            offset,
            paginated,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn get_organization_suggestions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        status: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationSuggestions,
        Error<organizations_memberships_api::GetOrganizationSuggestionsError>,
    > {
        let response = organizations_memberships_api::get_organization_suggestions(
            &self.config,
            limit,
            offset,
            status,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn get_users_organization_invitations(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        status: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedOrganizationInvitationsUserContext,
        Error<organizations_memberships_api::GetUsersOrganizationInvitationsError>,
    > {
        let response = organizations_memberships_api::get_users_organization_invitations(
            &self.config,
            limit,
            offset,
            status,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Passkeys API methods
    pub async fn attempt_passkey_verification(
        &self,
        passkey_id: &str,
        origin: Option<&str>,
        strategy: Option<&str>,
        public_key_credential: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedPasskey,
        Error<passkeys_api::AttemptPasskeyVerificationError>,
    > {
        let response = passkeys_api::attempt_passkey_verification(
            &self.config,
            passkey_id,
            origin,
            strategy,
            public_key_credential,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_passkey(
        &self,
        passkey_id: &str,
    ) -> Result<ClientPeriodClientWrappedDeletedObject, Error<passkeys_api::DeletePasskeyError>>
    {
        let response = passkeys_api::delete_passkey(&self.config, passkey_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn patch_passkey(
        &self,
        passkey_id: &str,
        name: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedPasskey, Error<passkeys_api::PatchPasskeyError>> {
        let response = passkeys_api::patch_passkey(&self.config, passkey_id, name).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn post_passkey(
        &self,
        _clerk_session_id: Option<&str>,
        origin: Option<&str>,
        x_original_host: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedPasskey, Error<passkeys_api::PostPasskeyError>> {
        let response =
            passkeys_api::post_passkey(&self.config, _clerk_session_id, origin, x_original_host)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn read_passkey(
        &self,
        passkey_id: &str,
    ) -> Result<ClientPeriodClientWrappedPasskey, Error<passkeys_api::ReadPasskeyError>> {
        let response = passkeys_api::read_passkey(&self.config, passkey_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Phone Numbers API methods
    pub async fn delete_phone_number(
        &self,
        phone_number_id: &str,
        clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<phone_numbers_api::DeletePhoneNumberError>,
    > {
        let response =
            phone_numbers_api::delete_phone_number(&self.config, phone_number_id, clerk_session_id)
                .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_phone_numbers(
        &self,
        clerk_session_id: Option<&str>,
    ) -> Result<Vec<ClientPeriodPhoneNumber>, Error<phone_numbers_api::GetPhoneNumbersError>> {
        phone_numbers_api::get_phone_numbers(&self.config, clerk_session_id).await
    }

    pub async fn post_phone_numbers(
        &self,
        phone_number: &str,
        _clerk_session_id: Option<&str>,
        reserved_for_second_factor: Option<bool>,
    ) -> Result<ClientPeriodClientWrappedPhoneNumber, Error<phone_numbers_api::PostPhoneNumbersError>>
    {
        let response = phone_numbers_api::post_phone_numbers(
            &self.config,
            phone_number,
            _clerk_session_id,
            reserved_for_second_factor,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn read_phone_number(
        &self,
        phone_number_id: &str,
        clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedPhoneNumber, Error<phone_numbers_api::ReadPhoneNumberError>>
    {
        let response =
            phone_numbers_api::read_phone_number(&self.config, phone_number_id, clerk_session_id)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn send_verification_sms(
        &self,
        phone_number_id: &str,
        strategy: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedPhoneNumber,
        Error<phone_numbers_api::SendVerificationSmsError>,
    > {
        let response = phone_numbers_api::send_verification_sms(
            &self.config,
            phone_number_id,
            strategy,
            _clerk_session_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn update_phone_number(
        &self,
        phone_number_id: &str,
        clerk_session_id: Option<&str>,
        reserved_for_second_factor: Option<bool>,
        default_second_factor: Option<bool>,
    ) -> Result<
        ClientPeriodClientWrappedPhoneNumber,
        Error<phone_numbers_api::UpdatePhoneNumberError>,
    > {
        let response = phone_numbers_api::update_phone_number(
            &self.config,
            phone_number_id,
            clerk_session_id,
            reserved_for_second_factor,
            default_second_factor,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn verify_phone_number(
        &self,
        phone_number_id: &str,
        code: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedPhoneNumber,
        Error<phone_numbers_api::VerifyPhoneNumberError>,
    > {
        let response = phone_numbers_api::verify_phone_number(
            &self.config,
            phone_number_id,
            code,
            _clerk_session_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Roles API methods
    pub async fn list_organization_roles(
        &self,
        organization_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<ClientPeriodClientWrappedRoles, Error<roles_api::ListOrganizationRolesError>> {
        let response =
            roles_api::list_organization_roles(&self.config, organization_id, limit, offset)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // SAML API methods
    pub async fn acs(&self, saml_connection_id: &str) -> Result<(), Error<saml_api::AcsError>> {
        saml_api::acs(&self.config, saml_connection_id).await
    }

    pub async fn saml_metadata(
        &self,
        saml_connection_id: &str,
    ) -> Result<(), Error<saml_api::SamlMetadataError>> {
        saml_api::saml_metadata(&self.config, saml_connection_id).await
    }

    // Sessions API methods
    pub async fn create_session_token(
        &self,
        session_id: &str,
        organization_id: Option<&str>,
    ) -> Result<CreateSessionToken200Response, Error<sessions_api::CreateSessionTokenError>> {
        sessions_api::create_session_token(&self.config, session_id, organization_id).await
    }

    pub async fn create_session_token_with_template(
        &self,
        session_id: &str,
        template_name: &str,
    ) -> Result<
        CreateSessionToken200Response,
        Error<sessions_api::CreateSessionTokenWithTemplateError>,
    > {
        sessions_api::create_session_token_with_template(&self.config, session_id, template_name)
            .await
    }

    pub async fn end_session(
        &self,
        session_id: &str,
    ) -> Result<ClientPeriodClientWrappedSession, Error<sessions_api::EndSessionError>> {
        let response = sessions_api::end_session(&self.config, session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<ClientPeriodClientWrappedSession, Error<sessions_api::GetSessionError>> {
        let response = sessions_api::get_session(&self.config, session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn remove_client_sessions_and_retain_cookie(
        &self,
    ) -> Result<
        ClientPeriodDeleteSession,
        Error<sessions_api::RemoveClientSessionsAndRetainCookieError>,
    > {
        let response = sessions_api::remove_client_sessions_and_retain_cookie(&self.config).await?;
        if let Some(client) = response.response.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn remove_session(
        &self,
        session_id: &str,
    ) -> Result<ClientPeriodClientWrappedSession, Error<sessions_api::RemoveSessionError>> {
        let response = sessions_api::remove_session(&self.config, session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn touch_session(
        &self,
        session_id: &str,
        active_organization_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSession, Error<sessions_api::TouchSessionError>> {
        let response =
            sessions_api::touch_session(&self.config, session_id, active_organization_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    // Sign Ins API methods
    pub async fn accept_ticket(
        &self,
        ticket: &str,
    ) -> Result<(), Error<sign_ins_api::AcceptTicketError>> {
        sign_ins_api::accept_ticket(&self.config, ticket).await
    }

    pub async fn attempt_sign_in_factor_one(
        &self,
        sign_in_id: &str,
        strategy: &str,
        origin: Option<&str>,
        code: Option<&str>,
        password: Option<&str>,
        signature: Option<&str>,
        token: Option<&str>,
        ticket: Option<&str>,
        public_key_credential: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::AttemptSignInFactorOneError>>
    {
        let response = sign_ins_api::attempt_sign_in_factor_one(
            &self.config,
            sign_in_id,
            strategy,
            origin,
            code,
            password,
            signature,
            token,
            ticket,
            public_key_credential,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    pub async fn attempt_sign_in_factor_two(
        &self,
        sign_in_id: &str,
        strategy: Option<&str>,
        code: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::AttemptSignInFactorTwoError>>
    {
        let response =
            sign_ins_api::attempt_sign_in_factor_two(&self.config, sign_in_id, strategy, code)
                .await?;

        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn create_sign_in(
        &self,
        origin: Option<&str>,
        strategy: Option<&str>,
        identifier: Option<&str>,
        password: Option<&str>,
        ticket: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        transfer: Option<bool>,
        code: Option<&str>,
        token: Option<&str>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::CreateSignInError>> {
        let response = sign_ins_api::create_sign_in(
            &self.config,
            origin,
            strategy,
            identifier,
            password,
            ticket,
            redirect_url,
            action_complete_redirect_url,
            transfer,
            code,
            token,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn get_sign_in(
        &self,
        sign_in_id: &str,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::GetSignInError>> {
        let response = sign_ins_api::get_sign_in(&self.config, sign_in_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn prepare_sign_in_factor_one(
        &self,
        sign_in_id: &str,
        strategy: &str,
        origin: Option<&str>,
        email_address_id: Option<&str>,
        phone_number_id: Option<&str>,
        web3_wallet_id: Option<&str>,
        passkey_id: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::PrepareSignInFactorOneError>>
    {
        let response = sign_ins_api::prepare_sign_in_factor_one(
            &self.config,
            sign_in_id,
            strategy,
            origin,
            email_address_id,
            phone_number_id,
            web3_wallet_id,
            passkey_id,
            redirect_url,
            action_complete_redirect_url,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn prepare_sign_in_factor_two(
        &self,
        sign_in_id: &str,
        strategy: Option<&str>,
        phone_number_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::PrepareSignInFactorTwoError>>
    {
        let response = sign_ins_api::prepare_sign_in_factor_two(
            &self.config,
            sign_in_id,
            strategy,
            phone_number_id,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn reset_password(
        &self,
        sign_in_id: &str,
        password: &str,
        sign_out_of_other_sessions: Option<bool>,
    ) -> Result<ClientPeriodClientWrappedSignIn, Error<sign_ins_api::ResetPasswordError>> {
        let response = sign_ins_api::reset_password(
            &self.config,
            sign_in_id,
            password,
            sign_out_of_other_sessions,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn verify(&self, token: &str) -> Result<(), Error<sign_ins_api::VerifyError>> {
        sign_ins_api::verify(&self.config, token).await
    }

    // Sign Ups API methods
    pub async fn attempt_sign_ups_verification(
        &self,
        sign_up_id: &str,
        origin: Option<&str>,
        strategy: Option<&str>,
        code: Option<&str>,
        signature: Option<&str>,
        token: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignUp, Error<sign_ups_api::AttemptSignUpsVerificationError>>
    {
        let response = sign_ups_api::attempt_sign_ups_verification(
            &self.config,
            sign_up_id,
            origin,
            strategy,
            code,
            signature,
            token,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn create_sign_ups(
        &self,
        origin: Option<&str>,
        transfer: Option<bool>,
        password: Option<&str>,
        first_name: Option<&str>,
        last_name: Option<&str>,
        username: Option<&str>,
        email_address: Option<&str>,
        phone_number: Option<&str>,
        email_address_or_phone_number: Option<&str>,
        unsafe_metadata: Option<&str>,
        strategy: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        redirect_url: Option<&str>,
        ticket: Option<&str>,
        web3_wallet: Option<&str>,
        token: Option<&str>,
        code: Option<&str>,
        captcha_token: Option<&str>,
        captcha_error: Option<&str>,
        captcha_widget_type: Option<&str>,
        legal_accepted: Option<bool>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignUp, Error<sign_ups_api::CreateSignUpsError>> {
        let response = sign_ups_api::create_sign_ups(
            &self.config,
            origin,
            transfer,
            password,
            first_name,
            last_name,
            username,
            email_address,
            phone_number,
            email_address_or_phone_number,
            unsafe_metadata,
            strategy,
            action_complete_redirect_url,
            redirect_url,
            ticket,
            web3_wallet,
            token,
            code,
            captcha_token,
            captcha_error,
            captcha_widget_type,
            legal_accepted,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn get_sign_ups(
        &self,
        sign_up_id: &str,
    ) -> Result<ClientPeriodClientWrappedSignUp, Error<sign_ups_api::GetSignUpsError>> {
        let response = sign_ups_api::get_sign_ups(&self.config, sign_up_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn prepare_sign_ups_verification(
        &self,
        sign_up_id: &str,
        origin: Option<&str>,
        strategy: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignUp, Error<sign_ups_api::PrepareSignUpsVerificationError>>
    {
        let response = sign_ups_api::prepare_sign_ups_verification(
            &self.config,
            sign_up_id,
            origin,
            strategy,
            redirect_url,
            action_complete_redirect_url,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    pub async fn update_sign_ups(
        &self,
        sign_up_id: &str,
        origin: Option<&str>,
        password: Option<&str>,
        first_name: Option<&str>,
        last_name: Option<&str>,
        username: Option<&str>,
        email_address: Option<&str>,
        phone_number: Option<&str>,
        email_address_or_phone_number: Option<&str>,
        unsafe_metadata: Option<&str>,
        strategy: Option<&str>,
        redirect_url: Option<&str>,
        action_complete_redirect_url: Option<&str>,
        ticket: Option<&str>,
        web3_wallet: Option<&str>,
        token: Option<&str>,
        code: Option<&str>,
        legal_accepted: Option<bool>,
        oidc_login_hint: Option<&str>,
        oidc_prompt: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedSignUp, Error<sign_ups_api::UpdateSignUpsError>> {
        let response = sign_ups_api::update_sign_ups(
            &self.config,
            sign_up_id,
            origin,
            password,
            first_name,
            last_name,
            username,
            email_address,
            phone_number,
            email_address_or_phone_number,
            unsafe_metadata,
            strategy,
            redirect_url,
            action_complete_redirect_url,
            ticket,
            web3_wallet,
            token,
            code,
            legal_accepted,
            oidc_login_hint,
            oidc_prompt,
        )
        .await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        };
        Ok(response)
    }

    // TOTP API methods
    pub async fn delete_totp(
        &self,
    ) -> Result<ClientPeriodClientWrappedDeletedObject, Error<totp_api::DeleteTotpError>> {
        let response = totp_api::delete_totp(&self.config).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn post_totp(
        &self,
    ) -> Result<ClientPeriodClientWrappedTotp, Error<totp_api::PostTotpError>> {
        let response = totp_api::post_totp(&self.config).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn verify_totp(
        &self,
        code: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedTotp, Error<totp_api::VerifyTotpError>> {
        let response = totp_api::verify_totp(&self.config, code).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // User API methods
    pub async fn create_service_token(
        &self,
        service: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<Token, Error<user_api::CreateServiceTokenError>> {
        user_api::create_service_token(&self.config, service, _clerk_session_id).await
    }

    pub async fn change_password(
        &self,
        new_password: &str,
        _clerk_session_id: Option<&str>,
        current_password: Option<&str>,
        sign_out_of_other_sessions: Option<bool>,
    ) -> Result<ClientPeriodClientWrappedUser, Error<user_api::ChangePasswordError>> {
        let response = user_api::change_password(
            &self.config,
            new_password,
            _clerk_session_id,
            current_password,
            sign_out_of_other_sessions,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_profile_image(
        &self,
        _clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedDeletedObject, Error<user_api::DeleteProfileImageError>>
    {
        let response = user_api::delete_profile_image(&self.config, _clerk_session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn delete_user(
        &self,
        _clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedDeletedObject, Error<user_api::DeleteUserError>> {
        let response = user_api::delete_user(&self.config, _clerk_session_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_user(
        &self,
        _clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedUser, Error<user_api::GetUserError>> {
        let response = user_api::get_user(&self.config, _clerk_session_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn patch_user(
        &self,
        _clerk_session_id: Option<&str>,
        first_name: Option<&str>,
        last_name: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
        primary_email_address_id: Option<&str>,
        primary_phone_number_id: Option<&str>,
        primary_web3_wallet_id: Option<&str>,
        public_metadata: Option<&str>,
        private_metadata: Option<&str>,
        unsafe_metadata: Option<&str>,
        profile_image_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedUser, Error<user_api::PatchUserError>> {
        let response = user_api::patch_user(
            &self.config,
            _clerk_session_id,
            first_name,
            last_name,
            username,
            password,
            primary_email_address_id,
            primary_phone_number_id,
            primary_web3_wallet_id,
            public_metadata,
            private_metadata,
            unsafe_metadata,
            profile_image_id,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn remove_password(
        &self,
        current_password: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedUser, Error<user_api::RemovePasswordError>> {
        let response =
            user_api::remove_password(&self.config, current_password, _clerk_session_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    /// Does not work, file upload not implemented yet
    pub async fn update_profile_image(
        &self,
        _clerk_session_id: Option<&str>,
        _file: Option<std::path::PathBuf>,
    ) -> Result<ClientPeriodClientWrappedImage, Error<user_api::UpdateProfileImageError>> {
        let response =
            user_api::update_profile_image(&self.config, _clerk_session_id, _file).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap();
        }
        Ok(response)
    }

    // Web3 Wallets API methods
    pub async fn attempt_web3_wallet_verification(
        &self,
        web3_wallet_id: &str,
        signature: &str,
        origin: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedWeb3Wallet,
        Error<web3_wallets_api::AttemptWeb3WalletVerificationError>,
    > {
        let response = web3_wallets_api::attempt_web3_wallet_verification(
            &self.config,
            web3_wallet_id,
            signature,
            origin,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn delete_web3_wallet(
        &self,
        web3_wallet_id: &str,
    ) -> Result<
        ClientPeriodClientWrappedDeletedObject,
        Error<web3_wallets_api::DeleteWeb3WalletError>,
    > {
        let response = web3_wallets_api::delete_web3_wallet(&self.config, web3_wallet_id).await?;
        if let Some(client) = response.client.clone() {
            self.handle_client_update(*client).unwrap()
        }
        Ok(response)
    }

    pub async fn get_web3_wallets(
        &self,
        clerk_session_id: Option<&str>,
    ) -> Result<Vec<ClientPeriodWeb3Wallet>, Error<web3_wallets_api::GetWeb3WalletsError>> {
        web3_wallets_api::get_web3_wallets(&self.config, clerk_session_id).await
    }

    pub async fn post_web3_wallets(
        &self,
        web3_wallet: &str,
        _clerk_session_id: Option<&str>,
    ) -> Result<ClientPeriodClientWrappedWeb3Wallet, Error<web3_wallets_api::PostWeb3WalletsError>>
    {
        let response =
            web3_wallets_api::post_web3_wallets(&self.config, web3_wallet, _clerk_session_id)
                .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn prepare_web3_wallet_verification(
        &self,
        web3_wallet_id: &str,
        strategy: &str,
        origin: Option<&str>,
        redirect_url: Option<&str>,
    ) -> Result<
        ClientPeriodClientWrappedWeb3Wallet,
        Error<web3_wallets_api::PrepareWeb3WalletVerificationError>,
    > {
        let response = web3_wallets_api::prepare_web3_wallet_verification(
            &self.config,
            web3_wallet_id,
            strategy,
            origin,
            redirect_url,
        )
        .await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    pub async fn read_web3_wallet(
        &self,
        web3_wallet_id: &str,
    ) -> Result<ClientPeriodClientWrappedWeb3Wallet, Error<web3_wallets_api::ReadWeb3WalletError>>
    {
        let response = web3_wallets_api::read_web3_wallet(&self.config, web3_wallet_id).await?;
        self.handle_client_update(*response.client.clone()).unwrap();
        Ok(response)
    }

    // Well Known API methods
    pub async fn get_android_asset_links(
        &self,
    ) -> Result<Vec<serde_json::Value>, Error<well_known_api::GetAndroidAssetLinksError>> {
        well_known_api::get_android_asset_links(&self.config).await
    }

    pub async fn get_apple_app_site_association(
        &self,
    ) -> Result<
        WellKnownPeriodAppleAppSiteAssociation,
        Error<well_known_api::GetAppleAppSiteAssociationError>,
    > {
        well_known_api::get_apple_app_site_association(&self.config).await
    }

    pub async fn get_jwks(&self) -> Result<Jwks, Error<well_known_api::GetJwksError>> {
        well_known_api::get_jwks(&self.config).await
    }

    pub async fn get_open_id_configuration(
        &self,
    ) -> Result<
        WellKnownPeriodOpenIdConfiguration,
        Error<well_known_api::GetOpenIdConfigurationError>,
    > {
        well_known_api::get_open_id_configuration(&self.config).await
    }
}

// Add this implementation after the ClerkFapiClient struct definition
impl Default for ClerkFapiClient {
    fn default() -> Self {
        // Create default configuration
        let config = ClerkFapiConfiguration::default();

        // Create the client, using empty string as fallback in case of error
        Self::new(config).unwrap_or_else(|_| {
            // Create a minimal working client with default configuration
            let api_config = ApiConfiguration::new();
            Self {
                config: Arc::new(api_config),
                update_client_callback: None,
            }
        })
    }
}

// Add this test to verify the implementation
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_client() {
        let client = ClerkFapiClient::default();
        assert_eq!(client.config.base_path, "");
        assert!(client.config.user_agent.is_some());
        assert!(client.update_client_callback.is_none());
    }
}
