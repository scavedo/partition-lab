# Partition Lab

An interactive model of what happens to replicated data when the network splits.

**Live demo:** https://scavedo.github.io/partition-lab/

Three devices share one record: a note and a count of items sold. You can cut the links between them, keep working on each side, and reconnect. The lab runs in two modes:

- **Availability (AP):** every device always accepts writes. When devices can reach each other, they exchange full state and merge it with CRDT rules, so all copies converge after the network heals.
- **Consistency (CP):** a write must reach a majority (2 of 3). A cut-off device refuses writes, so the copies never conflict. Editing the note also needs an edit lock (a mutex) kept in the same majority-replicated state, which shows why locks and partitions don't mix.

All simulation logic is in Rust, compiled to WebAssembly. The page in `www/` only draws state and forwards clicks.

## What it demonstrates

| Scenario | What you see |
| --- | --- |
| Isolate C, sell on both sides, reconnect | The PN-counter keeps every sale; a plain last-write-wins number loses one |
| Isolate C, edit the note on both sides, reconnect | Every copy settles on the same note, chosen by Lamport stamp |
| CP mode, isolate C, write on C | Refused: C can't reach a majority |
| CP mode, take the lock on C, isolate C | Nobody can edit the note until the lock's lease is expired from the majority side |

## Code tour

Each idea has its own file in `src/`:

- `network.rs`: which links are up, and which devices can reach each other.
- `crdt.rs`: the CRDTs. A last-write-wins register ordered by Lamport stamp `(counter, device)`, and a PN-counter with one increment and one decrement slot per device.
- `availability.rs`: availability mode. Each device's replica, local writes with a Lamport clock, and merging a connected group with the CRDTs.
- `consistency.rs`: consistency mode. Majority writes, and catching up a device that was cut off.
- `lock.rs`: the edit lock, a mutex stored in the majority-replicated record, with lease expiry for a holder that can't be reached.
- `lib.rs`: `Lab`, which holds the state and sends each action to the right module for the current mode.
- `outcome.rs` and `view.rs`: what each action reports, and the snapshot the page draws.
- `wasm.rs`: the browser boundary. Plain arguments in, JSON out.
- `www/`: the page (HTML, CSS and a small JavaScript file).

## Tests

```bash
cargo test
```

- `tests/scenarios.rs`: each scenario above as a test.
- `tests/laws.rs`: property-based tests (proptest):
  - merge is commutative, associative and idempotent over random histories;
  - in AP mode, any sequence of writes and partitions converges after healing, with no sales lost;
  - in CP mode, two copies at the same version are always identical (no split brain), and healing brings every copy to the same version.

## Build and run locally

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
./build.sh
python3 -m http.server -d www 8000   # then open http://localhost:8000
```

Pushing to `main` runs the tests, builds the WebAssembly, and deploys to GitHub Pages (`.github/workflows/pages.yml`).

## Simplifications

This is a teaching model, not a sync engine. Sync is instant and sends whole states; real systems send deltas, track what each peer has seen (for example with version vectors), handle unreliable transports, and garbage-collect metadata. Consistency mode stands in for a real consensus protocol with a simple majority rule over a known, fixed set of three devices.

## License

MIT
