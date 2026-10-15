use crate::error::{AppError, AppResult};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::Arc;

/// Accepts any server certificate. Used for sslmode=prefer/require, which in
/// libpq (and TablePlus) encrypt without verifying the peer.
#[derive(Debug)]
struct NoVerify(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub fn make_tls(ssl_mode: &str, root_cert: Option<&str>) -> AppResult<tokio_postgres_rustls::MakeRustlsConnect> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verify = matches!(ssl_mode, "verify-ca" | "verify-full");
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| AppError::msg(e.to_string()))?;
    let config = if verify {
        let mut roots = rustls::RootCertStore::empty();
        if let Some(path) = root_cert.filter(|p| !p.trim().is_empty()) {
            let pem = std::fs::read(path)?;
            for cert in rustls_pemfile_certs(&pem)? {
                roots.add(cert).map_err(|e| AppError::msg(e.to_string()))?;
            }
        } else {
            let native = rustls_native_certs::load_native_certs();
            for cert in native.certs {
                let _ = roots.add(cert);
            }
        }
        builder.with_root_certificates(roots).with_no_client_auth()
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify(provider)))
            .with_no_client_auth()
    };
    Ok(tokio_postgres_rustls::MakeRustlsConnect::new(config))
}

fn rustls_pemfile_certs(pem: &[u8]) -> AppResult<Vec<CertificateDer<'static>>> {
    // Minimal PEM parser (avoids an extra crate): collects CERTIFICATE blocks.
    let text = String::from_utf8_lossy(pem);
    let mut out = Vec::new();
    let mut in_block = false;
    let mut b64 = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("-----BEGIN CERTIFICATE") {
            in_block = true;
            b64.clear();
        } else if l.starts_with("-----END CERTIFICATE") {
            in_block = false;
            let der = base64_decode(&b64).ok_or_else(|| AppError::msg("invalid PEM certificate"))?;
            out.push(CertificateDer::from(der));
        } else if in_block {
            b64.push_str(l);
        }
    }
    if out.is_empty() {
        return Err(AppError::msg("no CERTIFICATE blocks found in root cert file"));
    }
    Ok(out)
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = T.iter().position(|&t| t == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}
