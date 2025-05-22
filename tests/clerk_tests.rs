#![recursion_limit = "512"]

use clerk_fapi_rs::clerk::Clerk;
use clerk_fapi_rs::configuration::ClerkFapiConfiguration;
use mockito::Server;
use serde_json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

    clerk.load(false).await.unwrap();

    env_mock.assert_async().await;
    client_mock.assert_async().await;
    assert!(clerk.environment().is_ok());
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
    let result = client.load(false).await;
    assert!(result.is_err());

    // Verify the mock was called
    env_mock.assert_async().await;
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
    client.load(false).await.unwrap();

    // Verify all mocks were called
    env_mock.assert_async().await;
    client_mock.assert_async().await;

    // Verify all state was set
    assert!(client.loaded());
    assert!(client.environment().is_ok());
    assert!(client.client().is_ok());
    assert!(client.session().unwrap().is_some());
    assert!(client.user().unwrap().is_some());
}

#[tokio::test]
async fn test_get_token() {
    let mut server = Server::new_async().await;

    // Mock the client endpoint
    let client_mock = server
        .mock("GET", "/v1/client?_is_native=1")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "response": {
                    "id": "test_client",
                    "object": "client",
                    "sign_in": null,
                    "sign_up": null,
                    "sessions": [{
                        "id": "sess_123",
                        "object": "session",
                        "status": "active",
                        "expire_at": 1804067200,
                        "abandon_at": 1904067200,
                        "last_active_at": 1704067200,
                        "last_active_organization_id": null,
                        "actor": null,
                        "user": {
                            "id": "user_123",
                            "object": "user",
                            "username": "testuser",
                            "first_name": "Test",
                            "last_name": "User",
                            "image_url": "",
                            "has_image": false,
                            "primary_email_address_id": "email_123",
                            "primary_phone_number_id": null,
                            "primary_web3_wallet_id": null,
                            "password_enabled": false,
                            "two_factor_enabled": false,
                            "totp_enabled": false,
                            "backup_code_enabled": false,
                            "email_addresses": [],
                            "phone_numbers": [],
                            "web3_wallets": [],
                            "passkeys": [],
                            "external_accounts": [],
                            "saml_accounts": [],
                            "public_metadata": {},
                            "external_id": null,
                            "last_sign_in_at": 1704067200,
                            "banned": false,
                            "locked": false,
                            "lockout_expires_in_seconds": null,
                            "verification_attempts_remaining": 100,
                            "created_at": 1704067200,
                            "updated_at": 1704067200,
                            "delete_self_enabled": true,
                            "create_organization_enabled": true,
                            "last_active_at": 1704067200,
                            "mfa_enabled_at": null,
                            "mfa_disabled_at": null,
                            "legal_accepted_at": null
                        },
                        "public_user_data": {
                            "first_name": "Test",
                            "last_name": "User",
                            "image_url": "",
                            "has_image": false,
                            "identifier": "testuser",
                            "profile_image_url": null
                        },
                        "factor_verification_age": [0],
                        "created_at": 1704067200,
                        "updated_at": 1704067200
                    }],
                    "last_active_session_id": "sess_123",
                    "cookie_expires_at": null,
                    "captcha_bypass": false,
                    "created_at": 1704067200,
                    "updated_at": 1704067200
                },
                "client": null
            })
            .to_string(),
        )
        .create_async()
        .await;

    // Mock the environment endpoint
    let env_mock = server
        .mock("GET", "/v1/environment?_is_native=1")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "auth_config": {
                    "object": "auth_config",
                    "id": "aac_test",
                    "first_name": "on",
                    "last_name": "on",
                    "email_address": "on",
                    "phone_number": "off",
                    "username": "on",
                    "password": "required",
                    "identification_requirements": [
                        ["email_address"]
                    ],
                    "identification_strategies": ["email_address"],
                    "first_factors": ["email_code"],
                    "second_factors": [],
                    "email_address_verification_strategies": ["email_code"],
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
                    "id": "display_config_test",
                    "instance_environment_type": "production",
                    "application_name": "test",
                    "theme": {
                        "buttons": {},
                        "general": {},
                        "accounts": {}
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
                    "support_email": "support@example.com",
                    "privacy_policy_url": "",
                    "terms_url": "",
                    "logo_url": "",
                    "favicon_url": "",
                    "branded": false,
                    "experimental_force_oauth_first": false,
                    "clerk_js_version": "4",
                    "show_devmode_warning": false,
                    "google_one_tap_client_id": null,
                    "help_url": null,
                    "captcha_public_key": null,
                    "captcha_widget_type": null,
                    "captcha_public_key_invisible": null,
                    "captcha_provider": null,
                    "captcha_oauth_bypass": [],
                    "logo_image": null,
                    "favicon_image": null
                },
                "user_settings": {
                    "attributes": {
                        "email_address": {
                            "enabled": true,
                            "required": true,
                            "used_for_first_factor": true,
                            "first_factors": ["email_code"],
                            "used_for_second_factor": false,
                            "second_factors": [],
                            "verifications": ["email_code"],
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
                            "second_factors": ["totp"],
                            "verifications": ["totp"],
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
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

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

    // Load the client state properly
    client.load(false).await.unwrap();

    // Test successful token creation
    let token = client.get_token(None, None).await.unwrap();
    assert_eq!(token, Some("test.jwt.token".to_string()));

    // Verify all mocks were called
    client_mock.assert_async().await;
    env_mock.assert_async().await;
    token_mock.assert_async().await;
}

#[tokio::test]
async fn test_listener() {
    let mut server = Server::new_async().await;

    // Mock the client endpoint for update_client
    let client_mock = server
        .mock("GET", "/v1/client?_is_native=1")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "response": {
                    "id": "test_client",
                    "object": "client",
                    "sign_in": null,
                    "sign_up": null,
                    "sessions": [{
                        "id": "test_session",
                        "object": "session",
                        "status": "active",
                        "expire_at": 1804067200,
                        "abandon_at": 1904067200,
                        "last_active_at": 1704067200,
                        "last_active_organization_id": null,
                        "actor": null,
                        "user": {
                            "id": "user_123",
                            "object": "user",
                            "username": "testuser",
                            "first_name": "Test",
                            "last_name": "User",
                            "image_url": "",
                            "has_image": false,
                            "primary_email_address_id": "email_123",
                            "primary_phone_number_id": null,
                            "primary_web3_wallet_id": null,
                            "password_enabled": false,
                            "two_factor_enabled": false,
                            "totp_enabled": false,
                            "backup_code_enabled": false,
                            "email_addresses": [],
                            "phone_numbers": [],
                            "web3_wallets": [],
                            "passkeys": [],
                            "external_accounts": [],
                            "saml_accounts": [],
                            "public_metadata": {},
                            "external_id": null,
                            "last_sign_in_at": 1704067200,
                            "banned": false,
                            "locked": false,
                            "lockout_expires_in_seconds": null,
                            "verification_attempts_remaining": 100,
                            "created_at": 1704067200,
                            "updated_at": 1704067200,
                            "delete_self_enabled": true,
                            "create_organization_enabled": true,
                            "last_active_at": 1704067200,
                            "mfa_enabled_at": null,
                            "mfa_disabled_at": null,
                            "legal_accepted_at": null
                        },
                        "public_user_data": {
                            "first_name": "Test",
                            "last_name": "User",
                            "image_url": "",
                            "has_image": false,
                            "identifier": "testuser",
                            "profile_image_url": null
                        },
                        "factor_verification_age": [0],
                        "created_at": 1704067200,
                        "updated_at": 1704067200
                    }],
                    "last_active_session_id": "test_session",
                    "cookie_expires_at": null,
                    "captcha_bypass": false,
                    "created_at": 1704067200,
                    "updated_at": 1704067200
                },
                "client": null
            })
            .to_string(),
        )
        .create_async()
        .await;

    // Mock the environment endpoint
    let env_mock = server
        .mock("GET", "/v1/environment?_is_native=1")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "auth_config": {
                    "object": "auth_config",
                    "id": "aac_test",
                    "first_name": "on",
                    "last_name": "on",
                    "email_address": "on",
                    "phone_number": "off",
                    "username": "on",
                    "password": "required",
                    "identification_requirements": [
                        ["email_address"]
                    ],
                    "identification_strategies": ["email_address"],
                    "first_factors": ["email_code"],
                    "second_factors": [],
                    "email_address_verification_strategies": ["email_code"],
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
                    "id": "display_config_test",
                    "instance_environment_type": "production",
                    "application_name": "test",
                    "theme": {
                        "buttons": {},
                        "general": {},
                        "accounts": {}
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
                    "support_email": "support@example.com",
                    "privacy_policy_url": "",
                    "terms_url": "",
                    "logo_url": "",
                    "favicon_url": "",
                    "branded": false,
                    "experimental_force_oauth_first": false,
                    "clerk_js_version": "4",
                    "show_devmode_warning": false,
                    "google_one_tap_client_id": null,
                    "help_url": null,
                    "captcha_public_key": null,
                    "captcha_widget_type": null,
                    "captcha_public_key_invisible": null,
                    "captcha_provider": null,
                    "captcha_oauth_bypass": [],
                    "logo_image": null,
                    "favicon_image": null
                },
                "user_settings": {
                    "attributes": {
                        "email_address": {
                            "enabled": true,
                            "required": true,
                            "used_for_first_factor": true,
                            "first_factors": ["email_code"],
                            "used_for_second_factor": false,
                            "second_factors": [],
                            "verifications": ["email_code"],
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
                            "second_factors": ["totp"],
                            "verifications": ["totp"],
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
                }
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

    // Load the client to trigger the callbacks
    clerk.load(false).await.unwrap();

    // Verify listener was called
    assert!(was_called.load(Ordering::SeqCst));

    // Verify all mocks were called
    client_mock.assert_async().await;
    env_mock.assert_async().await;
}
