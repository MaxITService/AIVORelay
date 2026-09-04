/// Select a provider before any HTTP/WebSocket client is created. The combined
/// dependency graph enables both ring and aws-lc, so Rustls cannot infer one.
/// This must also run when speech-only startup skips the TTS managers.
pub fn initialize() {
    // A provider installed earlier by the host is also valid. Installation is
    // process-wide and first-writer-wins; repeated calls are harmless.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    #[test]
    fn tls_client_can_start_without_tts_or_webview_initialization() {
        super::initialize();
        super::initialize();

        // This is the default-provider path used by tokio-tungstenite. It used
        // to panic when both crypto backends were enabled in speech-only mode.
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        let server_name = "localhost".try_into().unwrap();
        assert!(rustls::ClientConnection::new(std::sync::Arc::new(config), server_name).is_ok());
    }
}
