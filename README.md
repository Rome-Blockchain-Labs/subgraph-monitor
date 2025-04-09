# subgraph-monitor

rust implementation to monitor subgraph endpoint exposing prometheus metrics.

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
