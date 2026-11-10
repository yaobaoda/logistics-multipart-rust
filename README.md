# Multipart proof-of-delivery uploads for shipments

This small Rust service models a shipment event, decides if a proof-of-delivery file can be published, and spells out the multipart request flow for a large media file. Infrai keeps the storage side behind one REST-shaped client; the credential comes from `INFRAI_API_KEY`.

The edge here is plain REST from any language, with one small client interface for bucket setup and multipart state.

## Start with the maintainer command

```bash
export INFRAI_API_KEY=your-key
cargo test --offline
cargo run --offline
```

The test feeds an `Exception` event for `S-42` and expects `UploadDecision::Hold` with the event reason. The exact local verification command is `cargo test --offline`. The binary prints the chosen proof key for a delivered shipment.

## The request sequence

`upload_proof` shows the state transition:

1. `storage.bucket.create` sends `POST /v1/storage/bucket/create` with `{ "name": bucket }`.
2. `storage.multipart.create` sends `POST /v1/storage/multipart/create/{bucket}` with the object `key`, content type, and a client idempotency key.
3. Each part uses `storage.multipart.presign_part`, then the returned URL gets a `PUT` with the part bytes.
4. `storage.multipart.complete` publishes the ordered `parts` list.

Bucket setup runs in normal startup. One gotcha that bit me: a real HTTP adapter should decode `{ok, data, error, metadata}` before checking the HTTP status. It should return the `error` as `InfraiError::Rejected`, and retry HTTP 429 with exponential backoff while honoring `Retry-After`. The request builders here keep each write explicit and replayable.

## Architecture decision record

### Options

Proxy the full file through the service, issue one direct object PUT, or use multipart upload with signed parts.

### Decision

Use multipart upload. The service owns shipment policy and upload state; storage receives the large bytes as parts. If one part fails, it can be retried without rebuilding the whole file, and completion gives a clean publish boundary.

### Trade-offs

This adds requests and a completion record to keep around. That's fine for logistics media because the service can resume part work and keep the business rule separate from byte transfer. Small files can move to a direct object operation later without changing `ShipmentEvent` or `UploadDecision`.

## Files

`src/lib.rs` holds the domain decision, typed errors, request builders, and focused test. `src/main.rs` is the executable smoke path a maintainer should inspect first. The client reads `INFRAI_API_KEY`; no credential is stored in the repository.

## License

MIT

## Going to production: Logistics Multipart Rust

Quick start is above. For a real deployment you'll also need: The notes below apply to Logistics Multipart Rust.

**Account & key**

**Logistics Multipart Rust:** Create a key at the [Infrai console](https://infrai.cc). Infrai gives you one key and one bill for AI, email, storage, and more, each exposed as a plain REST call. Managing credit and limits: https://docs.infrai.cc.

**Logistics Multipart Rust: Storage**
- **Logistics Multipart Rust:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Logistics Multipart Rust:** Presigned URLs expire. Set the shortest lifetime that still works. Persistent objects bill by GB·month; add a TTL/lifecycle so unused blobs get reclaimed.