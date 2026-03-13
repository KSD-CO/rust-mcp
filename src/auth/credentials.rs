/// Raw credentials extracted from a request before validation.
///
/// This enum is transport-agnostic — it represents the normalized form of
/// whatever the client sent, regardless of whether the transport is HTTP/SSE,
/// stdio, or something else entirely.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Credentials {
    /// `Authorization: Bearer <token>` header.
    Bearer { token: String },

    /// `X-Api-Key: <key>` header, or `?api_key=<key>` query parameter.
    ApiKey { key: String },

    /// `Authorization: Basic <base64(username:password)>` header.
    Basic { username: String, password: String },

    /// A custom single-header credential.
    /// The header name is normalized to lowercase.
    CustomHeader { header_name: String, value: String },

    /// Verified TLS peer certificate (mTLS).
    /// Contains the DER-encoded bytes of the leaf certificate.
    ClientCertificate { der: Vec<u8> },

    /// No credentials were present in the request.
    /// Used to distinguish "unauthenticated request" from "invalid credentials".
    None,
}

impl Credentials {
    /// Returns `true` if no credentials were provided.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns a short label for logging/metrics (does not include secret values).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bearer { .. } => "bearer",
            Self::ApiKey { .. } => "api_key",
            Self::Basic { .. } => "basic",
            Self::CustomHeader { .. } => "custom_header",
            Self::ClientCertificate { .. } => "client_certificate",
            Self::None => "none",
        }
    }
}
