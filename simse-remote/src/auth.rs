use serde::Deserialize;

use crate::error::RemoteError;

// ── Auth state ──

#[derive(Debug, Clone)]
pub struct AuthState {
    pub user_id: String,
    pub session_token: String,
    pub team_id: Option<String>,
    pub role: Option<String>,
    pub api_url: String,
}

// ── API response types ──

#[derive(Debug, Deserialize)]
struct LoginResponse {
    data: LoginData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    session_token: String,
    user: LoginUser,
}

#[derive(Debug, Deserialize)]
struct LoginUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    data: ValidateData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateData {
    user_id: String,
    team_id: Option<String>,
    role: Option<String>,
}

// ── Auth client ──

pub struct AuthClient {
    http: reqwest::Client,
    state: Option<AuthState>,
}

impl Default for AuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            state: None,
        }
    }

    pub fn state(&self) -> Option<&AuthState> {
        self.state.as_ref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.state.is_some()
    }

    /// Login with email/password. Returns auth state on success.
    pub async fn login_password(
        &mut self,
        api_url: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthState, RemoteError> {
        let url = format!("{api_url}/auth/login");
        let res = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(RemoteError::AuthFailed(format!(
                "HTTP {status}: {body}"
            )));
        }

        let login_resp: LoginResponse = res.json().await?;

        // Validate the token to get team/role info
        let validate = self
            .validate_token(api_url, &login_resp.data.session_token)
            .await?;

        let state = AuthState {
            user_id: login_resp.data.user.id,
            session_token: login_resp.data.session_token,
            team_id: validate.team_id,
            role: validate.role,
            api_url: api_url.to_string(),
        };

        self.state = Some(state.clone());
        Ok(state)
    }

    /// Login with API key. Returns auth state on success.
    pub async fn login_api_key(
        &mut self,
        api_url: &str,
        api_key: &str,
    ) -> Result<AuthState, RemoteError> {
        let validate = self.validate_token(api_url, api_key).await?;

        let state = AuthState {
            user_id: validate.user_id,
            session_token: api_key.to_string(),
            team_id: validate.team_id,
            role: validate.role,
            api_url: api_url.to_string(),
        };

        self.state = Some(state.clone());
        Ok(state)
    }

    /// Logout: clear local state.
    pub fn logout(&mut self) {
        self.state = None;
    }

    /// Validate a token against the auth service.
    async fn validate_token(
        &self,
        api_url: &str,
        token: &str,
    ) -> Result<ValidateData, RemoteError> {
        let url = format!("{api_url}/auth/validate");
        let res = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "token": token }))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(RemoteError::AuthFailed(
                "Token validation failed".to_string(),
            ));
        }

        let validate: ValidateResponse = res.json().await?;
        Ok(validate.data)
    }

    /// Get current token, or error if not authenticated.
    pub fn require_auth(&self) -> Result<&AuthState, RemoteError> {
        self.state.as_ref().ok_or(RemoteError::NotAuthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_is_not_authenticated() {
        let client = AuthClient::new();
        assert!(!client.is_authenticated());
        assert!(client.state().is_none());
    }

    #[test]
    fn require_auth_fails_when_not_authenticated() {
        let client = AuthClient::new();
        let result = client.require_auth();
        assert!(result.is_err());
        match result.unwrap_err() {
            RemoteError::NotAuthenticated => {}
            other => panic!("expected NotAuthenticated, got {other}"),
        }
    }

    #[test]
    fn logout_clears_state() {
        let mut client = AuthClient::new();
        client.state = Some(AuthState {
            user_id: "u1".into(),
            session_token: "session_abc".into(),
            team_id: None,
            role: None,
            api_url: "https://api.example.com".into(),
        });
        assert!(client.is_authenticated());
        client.logout();
        assert!(!client.is_authenticated());
    }
}
