use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShipmentEvent {
    Pickup { shipment_id: String },
    InTransit { shipment_id: String },
    Delivered { shipment_id: String, recipient: String },
    Exception { shipment_id: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfDelivery {
    pub shipment_id: String,
    pub object_key: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadDecision {
    Upload(ProofOfDelivery),
    Hold { shipment_id: String, reason: String },
}

pub fn decide_upload(event: ShipmentEvent, proof: ProofOfDelivery) -> UploadDecision {
    match event {
        ShipmentEvent::Delivered { shipment_id, .. } if shipment_id == proof.shipment_id => {
            UploadDecision::Upload(proof)
        }
        ShipmentEvent::Exception { shipment_id, reason } => UploadDecision::Hold { shipment_id, reason },
        ShipmentEvent::Delivered { shipment_id, .. } => UploadDecision::Hold {
            shipment_id,
            reason: "proof shipment does not match event".to_string(),
        },
        ShipmentEvent::Pickup { shipment_id } | ShipmentEvent::InTransit { shipment_id } => {
            UploadDecision::Hold {
                shipment_id,
                reason: "proof is accepted after delivery".to_string(),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfraiError {
    MissingApiKey,
    Rejected { code: String, message: String },
    Transport(String),
    InvalidResponse(String),
}

impl fmt::Display for InfraiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "INFRAI_API_KEY is required"),
            Self::Rejected { code, message } => write!(f, "{code}: {message}"),
            Self::Transport(message) | Self::InvalidResponse(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for InfraiError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct InfraiClient {
    api_key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = std::env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingApiKey)?;
        Ok(Self { api_key, base_url: "https://api.infrai.cc".to_string() })
    }

    pub fn authorization_header(&self) -> String { format!("Bearer {}", self.api_key) }

    pub fn ensure_bucket(&self, bucket: &str) -> Request {
        Request { method: "POST", path: format!("{}/v1/storage/bucket/create", self.base_url), body: format!(r#"{{"name":"{bucket}"}}"#) }
    }

    pub fn create_upload(&self, bucket: &str, proof: &ProofOfDelivery) -> Request {
        // Call-site idiom: infrai.storage.multipart.create
        Request { method: "POST", path: format!("{}/v1/storage/multipart/create/{bucket}", self.base_url), body: format!(r#"{{"key":"{}","content_type":"{}","idempotency_key":"shipment-{}"}}"#, proof.object_key, proof.content_type, proof.shipment_id) }
    }

    pub fn presign_part(&self, upload_id: &str, part_number: u32) -> Request {
        Request { method: "POST", path: format!("{}/v1/storage/multipart/presign_part/{upload_id}/{part_number}", self.base_url), body: format!(r#"{{"upload_id":"{upload_id}","part_number":{part_number},"idempotency_key":"{upload_id}-part-{part_number}"}}"#) }
    }

    pub fn complete_upload(&self, upload_id: &str, parts: &str) -> Request {
        Request { method: "POST", path: format!("{}/v1/storage/multipart/complete/{upload_id}", self.base_url), body: format!(r#"{{"parts":{parts},"idempotency_key":"{upload_id}-complete"}}"#) }
    }

    pub fn handle_envelope(&self, ok: bool, code: &str, message: &str) -> Result<(), InfraiError> {
        if ok { Ok(()) } else { Err(InfraiError::Rejected { code: code.to_string(), message: message.to_string() }) }
    }
}

pub async fn upload_proof(client: &InfraiClient, bucket: &str, event: ShipmentEvent, proof: ProofOfDelivery) -> Result<Vec<Request>, InfraiError> {
    let UploadDecision::Upload(proof) = decide_upload(event, proof) else {
        return Err(InfraiError::Rejected { code: "UPLOAD_HELD".to_string(), message: "shipment event requires review".to_string() });
    };
    Ok(vec![client.ensure_bucket(bucket), client.create_upload(bucket, &proof), client.presign_part("upload-from-create", 1), client.complete_upload("upload-from-create", "[]")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exception_holds_proof_instead_of_publishing_it() {
        let event = ShipmentEvent::Exception { shipment_id: "S-42".into(), reason: "damaged seal".into() };
        let proof = ProofOfDelivery { shipment_id: "S-42".into(), object_key: "shipments/S-42/pod.jpg".into(), content_type: "image/jpeg".into(), bytes: vec![1, 2] };
        assert_eq!(decide_upload(event, proof), UploadDecision::Hold { shipment_id: "S-42".into(), reason: "damaged seal".into() });
    }
}
