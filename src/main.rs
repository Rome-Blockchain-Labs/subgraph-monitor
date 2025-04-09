use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use clap::Parser;
use prometheus::{IntGauge, Registry};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time;

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
            .service(health_endpoint)
            .service(metrics_endpoint)
    })
    .bind(("0.0.0.0", args.port))?
    .run()
    .await
}
