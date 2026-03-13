use anyhow::{Context, Result, bail, ensure};
use deterministic_rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use std::collections::{HashMap, VecDeque};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

pub type NodeId = [u8; 32];

pub struct StartConfig {
    pub port: u16,
    pub datadir: std::path::PathBuf,
    pub config_path: std::path::PathBuf,
}

struct RuntimeConfig {
    port: u16,
    datadir: std::path::PathBuf,
    signing_key: SigningKey,
    node_id: NodeId,
}

impl RuntimeConfig {
    pub fn from_config(path: PathBuf) -> Result<Self> {
        use sha2::Digest;

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
        // node id is the sha-2 hash of the verifying key
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

#[derive(Clone, PartialEq)]
pub struct NodeContact {
    //UDP port
    port: u16,
    node_id: NodeId,
    ip: std::net::IpAddr,
}

pub enum FindValueResult {
    Contact(Vec<NodeContact>),
    File(PathBuf),
}

pub struct Kademlia {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    routing_table: Vec<VecDeque<NodeContact>>,
    filepaths: HashMap<NodeId, PathBuf>,
    config: RuntimeConfig,
}

impl Kademlia {
    pub const BUCKET_SIZE: usize = 8;
    pub fn new(
        StartConfig {
            port,
            datadir,
            config_path,
        }: StartConfig,
    ) -> Result<Self> {
        let mut csprng = OsRng {};
        let signing = SigningKey::generate(&mut csprng);

        let key_hex = hex::encode(signing.as_bytes());

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

        Self::from_config(config_path)
    }

    pub fn from_config(config_path: PathBuf) -> Result<Self> {
        Ok(Kademlia {
            routing_table: (0..256).map(|_| VecDeque::with_capacity(Self::BUCKET_SIZE)).collect(),
            //OPTIMIZATION: add a floor to this that tells us what the first element of the routing table
            // with contacts in it is. chances are we're not gonna fill 0-200 in testing and even if
            // this grew to ipfs scale we'd still never fill most of them
            filepaths: HashMap::new(),
            config: RuntimeConfig::from_config(config_path)?,
        })
    }

    pub fn find_node(&self, node_id: NodeId) -> Result<Vec<NodeContact>> {
        ensure!(node_id != self.config.node_id, "trying to find ourself");
        // note: in this function (and elsewhere in this file) further/closer refer to ~~xor distance~~ which is described in the kademlia paper
        // all xor distance is is interpreting the size of a ^ b as the distance from a -> b.
        let routing_index = self.routing_index(node_id);

        // closer because nodes with an index >= routing index will always have a lower xor distance 
        // than nodes that have an index < routing index, see kademlia paper or just read routing_index() 
        // it's intuitive
        let closer_buckets = &self.routing_table[routing_index..self.routing_table.len()];
        let mut contacts: Vec<&NodeContact> = closer_buckets.iter().flatten().take(Self::BUCKET_SIZE).collect();

        if contacts.len() < Self::BUCKET_SIZE {
            let farther_buckets = &self.routing_table[0..routing_index];
            contacts.append(
                // here we take 16 because it guarantees that if we simply sort by xor distance later
                // we will get exactly the remaining closest nodes we know about, i believe any less
                // and we could get a suboptimal one
                &mut farther_buckets
                    .into_iter()
                    .rev()
                    .flatten()
                    .take(Self::BUCKET_SIZE * 2)
                    .collect(),
            );
        }

        contacts.sort_unstable_by(|a, b| {
            let dist_a = Self::xor_distance(a.node_id, node_id);
            let dist_b = Self::xor_distance(b.node_id, node_id);
            dist_a.cmp(&dist_b)
        });
        contacts.truncate(Self::BUCKET_SIZE);

        Ok(contacts.into_iter().cloned().collect())
    }

    pub fn find_value(&self, key: NodeId) -> Result<FindValueResult> {
        match self.filepaths.get(&key) {
            Some(path) => Ok(FindValueResult::File(path.to_owned())),
            None => Ok(FindValueResult::Contact(self.find_node(key)?)),
        }
    }

    /// update the routing table when we communicate with a
    /// node, confirming that it's alive
    pub fn update_bucket(&mut self, contact: NodeContact) {
        //TODO: liveness check here where we ping before evicting
        let i = self.routing_index(contact.node_id);

        if self.routing_table.get(i).is_none() {
            // this should never run but in case we optimize with lazily initing buckets later we will
            // have it for posterity sake
            let mut empty_bucket = VecDeque::new();
            empty_bucket.push_front(contact);
            self.routing_table.insert(i, empty_bucket);
            return;
        }

        let bucket = &mut self.routing_table[i];

        if bucket.len() < Self::BUCKET_SIZE {
            bucket.push_front(contact);
            return;
        }

        if let Some(pos) = bucket.iter().position(|known_contact| {
            known_contact.node_id == contact.node_id
        }) {
            // this implicitly allows for us to easily update ip addresses and ports in case of a quick reconfig,
            // allows for nice graceful disconnect/reconnect cause sometimes someone wants to turn on a vpn or
            // whatever
            bucket.remove(pos).unwrap();
        } else if bucket.len() == Self::BUCKET_SIZE {
            bucket.pop_back();       
        }

        bucket.push_front(contact);
        
        assert!(bucket.len() <= Self::BUCKET_SIZE);
    }

    // this is actually a good example of simple, idiomatic rust.
    // we're accepting any data structure that implements Read because this function only
    // needs to use .read() and it may introdue a burden on later development if we ever want to
    // write to this from anything that's not a BufReader.

    // this is especially important when you're writing a data structure
    pub fn store<R: std::io::Read>(
        &mut self,
        key: NodeId,
        reader: R,
        destination: PathBuf,
    ) -> Result<()> {
        let mut file_writer = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&destination)
            .with_context(|| format!("failed to open {} for writing", destination.display()))?;

        for byte in reader.bytes() {
            match byte {
                Ok(byte) => {
                    file_writer
                        .write_all(&[byte])
                        .with_context(|| format!("failed to write to {}", destination.display()))?;
                }
                Err(e) => match e.kind() {
                    ErrorKind::Interrupted => continue,
                    ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof => break,
                    // Something actually went wrong (e.g., disk full, permission denied)
                    _ => {
                        return Err(e).with_context(|| {
                            format!(
                                "failed while reading source bytes for {}",
                                destination.display()
                            )
                        });
                    }
                },
            }
        }

        file_writer
            .flush()
            .with_context(|| format!("failed to flush {}", destination.display()))?;

        self.filepaths.insert(key, destination);
        Ok(())
    }

    /// returns the number of matching leading bits of a node id and our node id
    fn routing_index(&self, id: NodeId) -> usize {
        assert!(
            id != self.config.node_id,
            "trying to find routing index of ourselves"
        );
        let mut i = 0;
        loop {
            if id[i] == self.config.node_id[i] {
                i += 1;
                continue;
            }
            let matching_high_bits: usize = (self.config.node_id[i] ^ id[i])
                .leading_zeros()
                .try_into()
                .unwrap();
            break (i * 8) + matching_high_bits;
        }
    }

    fn xor_distance(a: NodeId, b: NodeId) -> NodeId {
        let mut result: NodeId = [0u8; 32];
        for i in 0..a.len() {
            result[i] = a[i] ^ b[i];
        }
        result
    }
}
