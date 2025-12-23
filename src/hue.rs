use anyhow::Result;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use dtls::config::{Config};
use dtls::conn::DTLSConn;
use dtls::cipher_suite::CipherSuiteId;

#[derive(Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub ip: String,
    pub username: String,
    pub clientkey: String,
    pub group_id: String,
    pub light_ids: Vec<u16>, // V1 Integer IDs for streaming
}

pub struct Bridge {
    pub config: BridgeConfig,
    client: Client,
}

#[derive(Deserialize)]
struct DiscoveryResponse {
    internalipaddress: String,
}

#[derive(Deserialize)]
struct RegistrationResponse {
    success: Option<RegistrationSuccess>,
}

#[derive(Deserialize)]
struct RegistrationSuccess {
    username: String,
    clientkey: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Group {
    #[serde(skip_deserializing, default)]
    pub id: String,
    pub name: String,
    pub lights: Vec<String>,
    #[serde(rename = "type")]
    pub group_type: String,
}

impl Bridge {
    pub fn new(config: BridgeConfig) -> Result<Self> {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()?;
        Ok(Self { config, client })
    }

    pub async fn discover() -> Result<String> {
        let client = Client::new();
        let resp = client
            .get("https://discovery.meethue.com/")
            .send()
            .await?
            .json::<Vec<DiscoveryResponse>>()
            .await?;

        resp.first()
            .map(|r| r.internalipaddress.clone())
            .ok_or_else(|| anyhow::anyhow!("No Hue Bridge found"))
    }

    pub async fn register(ip: &str) -> Result<(String, String)> {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()?;

        println!("Please press the link button on your Hue Bridge now! (Waiting 30s)");
        
        for _ in 0..30 {
            let body = serde_json::json!({
                "devicetype": "hyperhue#linux",
                "generateclientkey": true
            });

            let resp = client
                .post(format!("http://{}/api", ip))
                .json(&body)
                .send()
                .await;

            if let Ok(resp) = resp {
                let parsed: Result<Vec<RegistrationResponse>, _> = resp.json().await;
                if let Ok(items) = parsed {
                    if let Some(item) = items.first() {
                        if let Some(success) = &item.success {
                            if let Some(key) = &success.clientkey {
                                return Ok((success.username.clone(), key.clone()));
                            }
                        }
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(anyhow::anyhow!("Button not pressed in time or clientkey not generated"))
    }

    // Use V1 API to get groups because we need integer IDs for streaming
    pub async fn get_entertainment_groups(ip: &str, username: &str) -> Result<Vec<Group>> {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()?;
            
        let url = format!("http://{}/api/{}/groups", ip, username);
        let resp = client.get(&url).send().await?.json::<std::collections::HashMap<String, Group>>().await?;
        
        let mut groups: Vec<Group> = resp.into_iter()
            .map(|(id, mut g)| { g.id = id; g })
            .filter(|g| g.group_type == "Entertainment")
            .collect();
            
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(groups)
    }

    pub async fn start_stream(&self) -> Result<()> {
        // Note: Stream activation is done on the Group resource, NOT the Action resource
        let url = format!("https://{}/api/{}/groups/{}", self.config.ip, self.config.username, self.config.group_id);
        let body = serde_json::json!({
            "stream": { "active": true }
        });
        
        let resp = self.client.put(&url).json(&body).send().await?;
        let text = resp.text().await?;
        println!("Stream activation response: {}", text);
        
        if text.contains("error") {
            return Err(anyhow::anyhow!("Failed to activate stream: {}", text));
        }
        
        Ok(())
    }
    
    pub async fn stop_stream(&self) -> Result<()> {
        let url = format!("https://{}/api/{}/groups/{}", self.config.ip, self.config.username, self.config.group_id);
        let body = serde_json::json!({
            "stream": { "active": false }
        });
        
        self.client.put(&url).json(&body).send().await?;
        Ok(())
    }
}

pub struct HueStream {
    conn: Arc<DTLSConn>,
    sequence: u8,
}

impl HueStream {
    pub async fn connect(bridge_ip: &str, username: &str, clientkey: &str) -> Result<Self> {
        let psk = hex::decode(clientkey)?;
        let psk_identity = username.as_bytes().to_vec();

        let config = Config {
            psk: Some(Arc::new(move |_| Ok(psk.clone()))),
            psk_identity_hint: Some(psk_identity),
            cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
            insecure_skip_verify: true,
            ..Default::default()
        };

        let addr: SocketAddr = format!("{}:2100", bridge_ip).parse()?;
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(addr).await?;

        let conn = Arc::new(DTLSConn::new(Arc::new(socket), config, true, None).await?);

        Ok(Self {
            conn,
            sequence: 0,
        })
    }

    pub async fn send_colors(&mut self, lights: &[u16], r: u8, g: u8, b: u8) -> Result<()> {
        let mut buffer = Vec::with_capacity(16 + lights.len() * 9);

        // Header
        buffer.extend_from_slice(b"HueStream");
        buffer.extend_from_slice(&[
            0x01, 0x00,       // Version 1.0
            self.sequence,    // Sequence number
            0x00, 0x00,       // Reserved
            0x00,             // Color Space (0 = RGB)
            0x00,             // Reserved
        ]);

        // Light Data
        for id in lights {
            buffer.push(0x00); // Type (0 = Light)
            buffer.extend_from_slice(&(*id).to_be_bytes());
            buffer.push(r);
            buffer.push(r);
            buffer.push(g);
            buffer.push(g);
            buffer.push(b);
            buffer.push(b);
        }

        self.conn.write(&buffer, None).await?;
        self.sequence = self.sequence.wrapping_add(1);
        
        Ok(())
    }
}
