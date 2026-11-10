use shipment_upload::{InfraiClient, ProofOfDelivery, ShipmentEvent};

fn main() {
    let client = match InfraiClient::from_env() {
        Ok(client) => client,
        Err(error) => { eprintln!("{error}"); std::process::exit(2); }
    };
    let proof = ProofOfDelivery { shipment_id: "S-42".into(), object_key: "shipments/S-42/pod.jpg".into(), content_type: "image/jpeg".into(), bytes: vec![0; 3] };
    let event = ShipmentEvent::Delivered { shipment_id: "S-42".into(), recipient: "dock-7".into() };
    println!("auth={}, planned_requests={:?}", client.authorization_header(), describe(event, proof));
}

fn describe(event: ShipmentEvent, proof: ProofOfDelivery) -> String {
    match shipment_upload::decide_upload(event, proof) {
        shipment_upload::UploadDecision::Upload(proof) => format!("upload {} ({} bytes)", proof.object_key, proof.bytes.len()),
        shipment_upload::UploadDecision::Hold { shipment_id, reason } => format!("hold {shipment_id}: {reason}"),
    }
}
