//NOTE: this file is vibe coded.
use crate::client::DolomedesClient;
use crate::kadem::{Kademlia, NodeContact, NodeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use crypto_bigint::U256;
use deterministic_rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use sha2::Digest;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

impl DolomedesClient {
    pub fn with_config(config_path: &Path) -> Result<Self> {
        let (port, datadir, signing_key, node_id) = read_config_file(config_path)?;
        let routing_table = Kademlia::new(node_id);
        let endpoint = make_endpoint(port)?;

        Ok(Self {
            port,
            datadir,
            signing_key,
            node_id,
            routing_table,
            seeders: Mutex::new(HashMap::new()),
            endpoint,
        })
    }
}

fn make_endpoint(port: u16) -> Result<quinn::Endpoint> {
    let server_config = make_server_config()?;
    let client_config = make_client_config();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    let mut endpoint = quinn::Endpoint::server(server_config, addr)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

fn make_server_config() -> Result<quinn::ServerConfig> {
    let cert = rcgen::generate_simple_self_signed(vec!["dolomedes".to_string()])?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());
    Ok(quinn::ServerConfig::with_single_cert(
        vec![cert_der],
        key_der,
    )?)
}

fn make_client_config() -> quinn::ClientConfig {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let crypto = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification(provider)))
        .with_no_client_auth();
    quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap(),
    ))
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub fn setup_env(config_path: PathBuf, datadir: PathBuf, port: u16) -> Result<()> {
    std::fs::create_dir_all(&datadir)
        .with_context(|| format!("failed to create datadir {}", datadir.display()))?;

    create_config_file(config_path, datadir, port)?;
    Ok(())
}

fn create_config_file(config_path: PathBuf, datadir: PathBuf, port: u16) -> Result<()> {
    let mut csprng = OsRng {};
    let signing_key = SigningKey::generate(&mut csprng);
    let key_hex = hex::encode(signing_key.as_bytes());

    let absolute_datadir = datadir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize datadir {}", datadir.display()))?;

    let content = format!(
        "secret_key={}\nport={}\ndatadir={}",
        key_hex,
        port,
        absolute_datadir
            .to_str()
            .context("datadir contains invalid UTF-8 and cannot be written to the config file")?,
    );

    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config_path)
        .with_context(|| format!("failed to create config file {}", config_path.display()))?
        .write_all(content.as_bytes())
        .with_context(|| format!("failed to write config file {}", config_path.display()))?;

    Ok(())
}

fn read_config_file(path: &Path) -> Result<(u16, PathBuf, SigningKey, NodeId)> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;

    let mut secret_key_hex: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut datadir: Option<PathBuf> = None;

    for (line_number, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').with_context(|| {
            format!(
                "invalid config line {} in {}: missing '='",
                line_number + 1,
                path.display()
            )
        })?;

        match key {
            "secret_key" => secret_key_hex = Some(value.to_string()),
            "port" => {
                port = Some(value.parse::<u16>().with_context(|| {
                    format!(
                        "invalid port value on line {} in {}",
                        line_number + 1,
                        path.display()
                    )
                })?)
            }
            "datadir" => datadir = Some(PathBuf::from(value)),
            _ => bail!(
                "unrecognized config key '{}' on line {} in {}",
                key,
                line_number + 1,
                path.display()
            ),
        }
    }

    let secret_key_hex = secret_key_hex.context("missing secret_key in config file")?;
    let secret_key: [u8; 32] = hex::decode(secret_key_hex)
        .context("secret_key is not valid hex")?
        .as_slice()
        .try_into()
        .context("secret_key must decode to exactly 32 bytes")?;

    let signing_key = SigningKey::from_bytes(&secret_key);
    let verifying_key = signing_key.verifying_key();
    let node_id = U256::from_be_slice(sha2::Sha256::digest(verifying_key.as_bytes()).as_slice());

    Ok((
        port.context("missing port in config file")?,
        datadir.context("missing datadir in config file")?,
        signing_key,
        node_id,
    ))
}

/// Parses a genesis file — one node per line, `#` comments allowed.
/// Format: `<ip>:<port>:<node_id_hex>`
/// Example: `192.168.1.1:31460:aabbcc...`
pub fn read_genesis_file(path: &Path) -> Result<Vec<NodeContact>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read genesis file at {}", path.display()))?;

    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            Some((i + 1, line))
        })
        .map(|(line_number, line)| {
            let err = || {
                format!(
                    "invalid genesis entry on line {} in {}: expected <ip>:<port>:<node_id_hex>",
                    line_number,
                    path.display()
                )
            };

            // split from the right so IPv6 addresses parse cleanly
            let (ip_and_port, node_id_hex) = line.rsplit_once(':').with_context(err)?;
            let (ip_str, port_str) = ip_and_port.rsplit_once(':').with_context(err)?;

            let ip: IpAddr = ip_str.parse().with_context(err)?;
            let port: u16 = port_str.parse().with_context(err)?;
            let node_id_bytes: [u8; 32] = hex::decode(node_id_hex)
                .with_context(err)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("{}", err()))?;
            let node_id = U256::from_be_slice(&node_id_bytes);

            Ok(NodeContact { ip, port, node_id })
        })
        .collect()
}
