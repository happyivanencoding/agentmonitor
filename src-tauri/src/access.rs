//! Validate the Cloudflare Access application JWT; never trust identity headers by themselves.
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub public_url: String,
    pub team_domain: String,
    pub audience: String,
}
impl Config {
    pub fn read(data: &Path) -> Result<Option<Self>, String> {
        let path = data.join("remote.json");
        if !path.exists() {
            return Ok(None);
        }
        let c: Self = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let url = url::Url::parse(&c.public_url).map_err(|_| "Invalid public URL")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("publicUrl must be an HTTPS origin".into());
        }
        if !c.team_domain.ends_with(".cloudflareaccess.com")
            || !c
                .team_domain
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            || c.audience.trim().is_empty()
        {
            return Err("Cloudflare team domain and application audience are required".into());
        }
        Ok(Some(c))
    }
    pub fn origin(&self) -> &str {
        self.public_url.trim_end_matches('/')
    }
    pub fn host(&self) -> String {
        url::Url::parse(&self.public_url)
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned()
    }
    fn validation(&self) -> Validation {
        let mut v = Validation::new(Algorithm::RS256);
        v.set_audience(&[&self.audience]);
        v.set_issuer(&[format!("https://{}", self.team_domain)]);
        v.set_required_spec_claims(&["exp", "iss", "aud"]);
        v.validate_nbf = true;
        v.leeway = 30;
        v
    }
}
struct CachedKeys {
    keys: Option<JwkSet>,
    loaded: Option<Instant>,
    attempted: Option<Instant>,
}
pub struct Access {
    pub config: Config,
    client: reqwest::blocking::Client,
    cache: Mutex<CachedKeys>,
}
impl Access {
    pub fn new(config: Config) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            config,
            client,
            cache: Mutex::new(CachedKeys {
                keys: None,
                loaded: None,
                attempted: None,
            }),
        })
    }
    pub fn validate(&self, token: &str) -> Result<Value, String> {
        let header = decode_header(token).map_err(|_| "Invalid Access token")?;
        if header.alg != Algorithm::RS256 {
            return Err("Invalid Access signing algorithm".into());
        }
        let kid = header.kid.ok_or("Missing Access signing key")?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| "Access key cache unavailable")?;
        let fresh = cache
            .loaded
            .is_some_and(|t| t.elapsed() < Duration::from_secs(3600));
        let known = cache.keys.as_ref().and_then(|k| k.find(&kid)).is_some();
        if !fresh || !known {
            if cache
                .attempted
                .is_none_or(|t| t.elapsed() > Duration::from_secs(30))
            {
                cache.attempted = Some(Instant::now());
                let keys = self
                    .client
                    .get(format!(
                        "https://{}/cdn-cgi/access/certs",
                        self.config.team_domain
                    ))
                    .send()
                    .and_then(|r| r.error_for_status())
                    .and_then(|r| r.json::<JwkSet>())
                    .map_err(|_| "Access signing keys temporarily unavailable")?;
                cache.keys = Some(keys);
                cache.loaded = Some(Instant::now());
            }
        }
        let jwk = cache
            .keys
            .as_ref()
            .and_then(|keys| keys.find(&kid))
            .ok_or("Unknown Access signing key")?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| "Invalid Access public key")?;
        decode::<Value>(token, &key, &self.config.validation())
            .map(|v| v.claims)
            .map_err(|_| "Access session invalid or expired".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_validation_requires_expected_app_and_time() {
        let c = Config {
            public_url: "https://monitor.example.com".into(),
            team_domain: "example.cloudflareaccess.com".into(),
            audience: "only-this-app".into(),
        };
        let v = c.validation();
        assert_eq!(v.algorithms, vec![Algorithm::RS256]);
        assert!(v.validate_exp && v.validate_nbf && v.validate_aud);
        assert!(v.aud.unwrap().contains("only-this-app"));
        assert!(v
            .iss
            .unwrap()
            .contains("https://example.cloudflareaccess.com"));
        assert!(v.required_spec_claims.contains("aud"));
    }
    #[test]
    fn forged_hmac_is_rejected_before_network() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        let token=encode(&Header::default(),&serde_json::json!({"exp":4102444800u64,"aud":"a","iss":"https://example.cloudflareaccess.com"}),&EncodingKey::from_secret(b"not-cloudflare")).unwrap();
        let access = Access::new(Config {
            public_url: "https://monitor.example.com".into(),
            team_domain: "example.cloudflareaccess.com".into(),
            audience: "a".into(),
        })
        .unwrap();
        assert!(access.validate(&token).unwrap_err().contains("algorithm"));
    }
}
