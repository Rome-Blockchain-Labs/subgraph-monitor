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

## deploy flow
you need to add the following variables and secrets to GitHub for this workflow:

### 🔐 **Secrets (under *Settings → Secrets and variables → Actions → Secrets*)**
- `SSH_PRIVATE_KEY` – Private SSH key for deployment access (used by `ssh-agent`).


### 🌐 **Variables (under *Settings → Secrets and variables → Actions → Variables*)**
- `SERVER_IP` – IP address of the target deployment server.
- `SSH_USER` – SSH username for deployment.
