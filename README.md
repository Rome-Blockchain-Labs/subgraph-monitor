# subgraph-monitor
rust implementation to monitor subgraph endpoint exposing prometheus metrics

## usage
```sh
curl -L https://github.com/Rome-Blockchain-Labs/subgraph-monitor/releases/download/v0.1.1/subgraph-monitor-x86_64 -o ~/subgraph-monitor && chmod +x ~/subgraph-monitor && ~/subgraph-monitor -h
```

## build
```sh
git clone https://github.com/Rome-Blockchain-Labs/subgraph-monitor.git
cd subgraph-monitor
cargo build --release
./target/release/subgraph-monitor -h
```

## monitoring
- exposes `/metrics` endpoint for prometheus scraping
- exposes `/health` endpoint returning 200 if healthy, 503 if not
- can be used with haproxy for failover using health check

## examples
check prometheus for alerts and haproxy for failover lb setup
