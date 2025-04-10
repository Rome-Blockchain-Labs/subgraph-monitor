use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use clap::Parser;
use prometheus::{IntGauge, Registry};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time;

mod dashboard;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Subgraph endpoint URL
    #[clap(short, long, default_value = "https://flare-query.sceptre.fi/subgraphs/name/sflr-subgraph")]
    endpoint: String,

    /// RPC endpoint URL
    #[clap(short, long, default_value = "https://flare.gateway.tenderly.co")]
    rpc: String,

    /// Port to run the monitor on
    #[clap(short, long, default_value_t = 3000)]
    port: u16,

    /// Check interval in seconds
    #[clap(short, long, default_value_t = 60)]
    interval: u64,
}

#[derive(Clone, Debug, Serialize)]
struct SubgraphStatus {
    healthy: bool,
    synced_block_height: i64,
    chain_head_block_height: i64,
    blocks_behind: i64,
    last_checked: String,
}

#[derive(Deserialize)]
struct GraphQLResponse {
    data: GraphQLData,
}

#[derive(Deserialize)]
struct GraphQLData {
    _meta: MetaData,
}

#[derive(Deserialize)]
struct MetaData {
    block: BlockData,
    #[serde(rename = "hasIndexingErrors")]
    has_indexing_errors: bool,
}

#[derive(Deserialize)]
struct BlockData {
    number: i64,
//    hash: String,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: String,
}

struct AppState {
    subgraph_url: String,
    rpc_url: String,
    status: Arc<Mutex<SubgraphStatus>>,
    registry: Registry,
    metrics: Arc<SubgraphMetrics>,
}

#[derive(Clone)]
struct SubgraphMetrics {
    healthy: IntGauge,
    synced_block: IntGauge,
    chain_head: IntGauge,
    blocks_behind: IntGauge,
}

async fn query_subgraph_status(client: &Client, url: &str) -> Result<GraphQLResponse, reqwest::Error> {
    let query = r#"{"query": "{_meta{block{number hash}hasIndexingErrors}}"}"#;

    let res = client.post(url)
        .header("Content-Type", "application/json")
        .body(query)
        .send()
        .await?
        .json::<GraphQLResponse>()
        .await?;

    Ok(res)
}

async fn query_chain_head(client: &Client, url: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let query = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;

    let res = client.post(url)
        .header("Content-Type", "application/json")
        .body(query)
        .send()
        .await?
        .json::<RpcResponse>()
        .await?;

    // convert hex to decimal
    let block_hex = res.result.trim_start_matches("0x");
    let block_number = i64::from_str_radix(block_hex, 16)?;

    Ok(block_number)
}

async fn check_subgraph(app_state: web::Data<AppState>) {
    let client = Client::new();
    
    // get current time before any async operations
    let formatted_time = chrono::Utc::now().to_rfc3339();
    
    // query subgraph status (outside of mutex lock)
    let subgraph_result = query_subgraph_status(&client, &app_state.subgraph_url).await;
    
    // only if successful, query chain head (outside of mutex lock)
    let chain_head_result = match &subgraph_result {
        Ok(_) => query_chain_head(&client, &app_state.rpc_url).await,
        Err(_) => Err("Skipping chain head query due to subgraph error".into()),
    };
    
    // process results and update state (no awaits from this point)
    let mut is_healthy = false;
    let mut synced_block = 0;
    let mut chain_head = 0;
    let mut blocks_behind = 0;
    
    // parse results outside the lock
    match subgraph_result {
        Ok(response) => {
            let meta = &response.data._meta;
            synced_block = meta.block.number;
            
            // check if the subgraph has indexing errors
            let has_indexing_errors = meta.has_indexing_errors;
            
            // process chain head result
            match chain_head_result {
                Ok(head) => {
                    chain_head = head;
                    blocks_behind = chain_head - synced_block;
                    
                    // determine health: no indexing errors and not too far behind
                    is_healthy = !has_indexing_errors && blocks_behind <= 20;
                    
                    println!(
                        "Subgraph check: Healthy={}, Synced block={}, Chain head={}, Blocks behind={}",
                        is_healthy, synced_block, chain_head, blocks_behind
                    );
                },
                Err(e) => {
                    eprintln!("Error getting chain head: {}", e);
                    // if we can't get chain head, rely only on indexing errors
                    is_healthy = !has_indexing_errors;
                }
            }
        },
        Err(e) => {
            eprintln!("Error querying subgraph: {}", e);
        }
    }
    
    // now update metrics and state with a short-lived lock
    {
        // update status with mutex lock (no awaits inside this block)
        let mut status = app_state.status.lock().unwrap();
        status.healthy = is_healthy;
        status.synced_block_height = synced_block;
        status.chain_head_block_height = chain_head;
        status.blocks_behind = blocks_behind;
        status.last_checked = formatted_time;
    }
    
    // update metrics (outside lock)
    app_state.metrics.healthy.set(if is_healthy { 1 } else { 0 });
    app_state.metrics.synced_block.set(synced_block);
    app_state.metrics.chain_head.set(chain_head);
    app_state.metrics.blocks_behind.set(blocks_behind);
}

#[get("/")]
async fn root(app_state: web::Data<AppState>) -> impl Responder {
    // minimize mutex lock duration by cloning only what's needed
    let status = {
        let status_guard = app_state.status.lock().unwrap();
        status_guard.clone()
    };

    let health_color = if status.healthy { "#22c55e" } else { "#ef4444" };
    let health_text = if status.healthy { "Healthy" } else { "Unhealthy" };
    
    HttpResponse::Ok().content_type("text/html").body(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Subgraph Monitor</title>
    <style>
        :root {{
            --bg-color: #f8fafc;
            --card-bg: #ffffff;
            --text-color: #1e293b;
            --border-color: #e2e8f0;
            --accent-color: #3b82f6;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background-color: var(--bg-color);
            color: var(--text-color);
            margin: 0;
            padding: 0;
            line-height: 1.5;
        }}
        .container {{
            max-width: 800px;
            margin: 0 auto;
            padding: 2rem;
        }}
        .card {{
            background-color: var(--card-bg);
            border-radius: 8px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.05);
            padding: 1.5rem;
            margin-bottom: 1.5rem;
        }}
        h1 {{
            margin: 0 0 1.5rem 0;
            font-weight: 600;
            font-size: 1.8rem;
            border-bottom: 2px solid var(--accent-color);
            padding-bottom: 0.5rem;
            color: var(--accent-color);
        }}
        p {{
            margin: 0.75rem 0;
        }}
        .info-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 1rem;
            margin: 1.5rem 0;
        }}
        .info-item {{
            padding: 1rem;
            background-color: #f1f5f9;
            border-radius: 6px;
            border-left: 4px solid var(--accent-color);
        }}
        .status-indicator {{
            font-weight: 600;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            background-color: {health_color};
            color: white;
            display: inline-block;
        }}
        .stat-label {{
            font-size: 0.875rem;
            color: #64748b;
            margin-bottom: 0.25rem;
        }}
        .stat-value {{
            font-size: 1.25rem;
            font-weight: 600;
        }}
        .metrics-links {{
            margin-top: 1.5rem;
            text-align: center;
        }}
        .metrics-links a {{
            display: inline-block;
            margin: 0 0.5rem;
            padding: 0.5rem 1rem;
            background-color: var(--accent-color);
            color: white;
            text-decoration: none;
            border-radius: 4px;
            font-weight: 500;
            transition: background-color 0.2s;
        }}
        .metrics-links a:hover {{
            background-color: #2563eb;
        }}
        .timestamp {{
            font-size: 0.875rem;
            color: #64748b;
            text-align: right;
            margin-top: 0.5rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>Subgraph Monitor</h1>
            
            <div class="info-grid">
                <div class="info-item">
                    <div class="stat-label">Subgraph</div>
                    <div class="stat-value" style="font-size: 0.9rem; word-break: break-all;">{}</div>
                </div>
                <div class="info-item">
                    <div class="stat-label">RPC Endpoint</div>
                    <div class="stat-value" style="font-size: 0.9rem; word-break: break-all;">{}</div>
                </div>
            </div>
            
            <div class="card">
                <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem;">
                    <span style="font-weight: 600; font-size: 1.2rem;">Status:</span>
                    <span class="status-indicator">{}</span>
                </div>
                
                <div class="info-grid">
                    <div class="info-item">
                        <div class="stat-label">Synced Block</div>
                        <div class="stat-value">{}</div>
                    </div>
                    <div class="info-item">
                        <div class="stat-label">Chain Head</div>
                        <div class="stat-value">{}</div>
                    </div>
                    <div class="info-item">
                        <div class="stat-label">Blocks Behind</div>
                        <div class="stat-value">{}</div>
                    </div>
                </div>
                
                <div class="timestamp">Last checked: {}</div>
            </div>
            
            <div class="metrics-links">
                <a href="/health">JSON Health Endpoint</a>
                <a href="/metrics">Prometheus Metrics</a>
            </div>
        </div>
    </div>
</body>
</html>"#,
        app_state.subgraph_url,
        app_state.rpc_url,
        health_text,
        status.synced_block_height,
        status.chain_head_block_height,
        status.blocks_behind,
        status.last_checked
    ))
}

#[get("/health")]
async fn health_endpoint(app_state: web::Data<AppState>) -> impl Responder {
    let status = app_state.status.lock().unwrap().clone();

    let status_code = if status.healthy { 200 } else { 503 };

    HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
        .content_type("application/json")
        .json(status)
}

#[get("/metrics")]
async fn metrics_endpoint(app_state: web::Data<AppState>) -> impl Responder {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = app_state.registry.gather();

    match encoder.encode_to_string(&metric_families) {
        Ok(metrics) => HttpResponse::Ok().content_type("text/plain").body(metrics),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("Subgraph Block Height Monitor");
    println!("-----------------------------");
    println!("Monitoring subgraph at: {}", args.endpoint);
    println!("Using RPC endpoint: {}", args.rpc);
    println!("Check interval: {} seconds", args.interval);
    println!("Server running at: http://localhost:{}", args.port);

    // create metrics
    let registry = Registry::new();

    let healthy_gauge = IntGauge::new("subgraph_healthy", "Whether the subgraph is healthy").unwrap();
    let synced_block_gauge = IntGauge::new("subgraph_synced_block", "The latest indexed block height").unwrap();
    let chain_head_gauge = IntGauge::new("subgraph_chain_head", "The current chain head block height").unwrap();
    let blocks_behind_gauge = IntGauge::new("subgraph_blocks_behind", "How many blocks behind the subgraph is").unwrap();

    registry.register(Box::new(healthy_gauge.clone())).unwrap();
    registry.register(Box::new(synced_block_gauge.clone())).unwrap();
    registry.register(Box::new(chain_head_gauge.clone())).unwrap();
    registry.register(Box::new(blocks_behind_gauge.clone())).unwrap();

    let metrics = Arc::new(SubgraphMetrics {
        healthy: healthy_gauge,
        synced_block: synced_block_gauge,
        chain_head: chain_head_gauge,
        blocks_behind: blocks_behind_gauge,
    });

    // initialize app state
    let app_state = web::Data::new(AppState {
        subgraph_url: args.endpoint.clone(),
        rpc_url: args.rpc.clone(),
        status: Arc::new(Mutex::new(SubgraphStatus {
            healthy: false,
            synced_block_height: 0,
            chain_head_block_height: 0,
            blocks_behind: 0,
            last_checked: "".to_string(),
        })),
        registry,
        metrics,
    });

    // clone for the background task
    let app_state_clone = app_state.clone();

    // start background task for checking subgraph
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(args.interval));

        // run initial check
        check_subgraph(app_state_clone.clone()).await;

        // schedule regular checks
        loop {
            interval.tick().await;
            check_subgraph(app_state_clone.clone()).await;
        }
    });

    // start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(web::resource("/").to(dashboard::render_dashboard))
            .service(health_endpoint)
            .service(metrics_endpoint)
    })
    .bind(("0.0.0.0", args.port))?
    .run()
    .await
}
