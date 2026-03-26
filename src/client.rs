/*
    we'll have:
    ping (self explanatory)
    find_node (returns k closest nodes)
    find_value (returns k closest nodes or value if it's stored here)
    store (upload a file)
*/

use anyhow::{Context, Result, bail};
use deterministic_rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use sha2::Digest;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;

use crate::kadem::{Kademlia, NodeContact, NodeId};

pub fn run(config_path: PathBuf, routing_table_path: Option<PathBuf>) -> Result<()> {
    let config = RuntimeConfig::from_config_file(config_path)?;

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let connection = join_network(addr)?;

    let routing_table = match routing_table_path {
        None => Kademlia::new(config.node_id, ping),
        Some(path) => Kademlia::from_file(path, ping)?,
    };

    Ok(())
}

fn join_network(addr: SocketAddr) -> Result<TcpStream> {
    todo!()
}

async fn ping(contact: &NodeContact) -> bool {
    todo!()
}

struct RuntimeConfig {
    pub port: u16,
    pub datadir: PathBuf,
    pub signing_key: SigningKey,
    pub node_id: NodeId,
}

impl RuntimeConfig {
    pub fn create(port: u16, datadir: PathBuf, config_path: PathBuf) -> Result<Self> {
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
            absolute_datadir.to_str().context(
                "datadir contains invalid UTF-8 and cannot be written to the config file"
            )?,
        );

        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_path)
            .with_context(|| format!("failed to create config file {}", config_path.display()))?
            .write_all(content.as_bytes())
            .with_context(|| format!("failed to write config file {}", config_path.display()))?;

        Self::from_config_file(config_path)
    }

    pub fn from_config_file(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file at {}", path.display()))?;

        let mut secret_key_hex: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut datadir: Option<PathBuf> = None;

        for (line_number, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
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
        let node_id: NodeId = sha2::Sha256::digest(verifying_key.as_bytes())
            .as_slice()
            .try_into()
            .expect("sha256 output must be 32 bytes");

        Ok(RuntimeConfig {
            port: port.context("missing port in config file")?,
            datadir: datadir.context("missing datadir in config file")?,
            signing_key,
            node_id,
        })
    }
}
