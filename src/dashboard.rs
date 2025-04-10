use actix_web::{web, HttpResponse, Responder};
use crate::AppState;
pub async fn render_dashboard(app_state: web::Data<AppState>) -> impl Responder {
    // minimize mutex lock duration by cloning only what's needed
    let status = {
        let status_guard = app_state.status.lock().unwrap();
        status_guard.clone()
    };
    let (health_color, health_text_color, health_text) = if status.healthy {
        ("#c9b16d", "#000000", "Healthy") // gold bg, black text
    } else {
        ("#c92d2d", "#ffffff", "Unhealthy") // rich red bg, white text
    };
    
    HttpResponse::Ok().content_type("text/html").body(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Subgraph Monitor</title>
    <style>
        :root {{
            --bg-color: #000000;
            --card-bg: #111111;
            --text-color: #ffffff;
            --muted-color: #aaaaaa;
            --panel-bg: #0a0a0a;
            --accent-color: #c9b16d;
            --footer-color: #151515;
            --grid-color: #222222;
            --hover-color: #222222;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background-color: var(--bg-color);
            background-image: linear-gradient(var(--grid-color) 1px, transparent 1px), 
                              linear-gradient(90deg, var(--grid-color) 1px, transparent 1px);
            background-size: 40px 40px;
            color: var(--text-color);
            margin: 0;
            padding: 0;
            line-height: 1.5;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }}
        .container {{
            max-width: 900px;
            margin: 0 auto;
            padding: 2rem;
            flex: 1;
            width: 100%;
            box-sizing: border-box;
        }}
        .card {{
            background-color: var(--card-bg);
            border-radius: 0px;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
            padding: 2rem;
            margin-bottom: 1.5rem;
            border-left: 4px solid var(--accent-color);
        }}
        h1 {{
            margin: 0 0 1.5rem 0;
            font-weight: 600;
            font-size: 2rem;
            border-bottom: 2px solid var(--accent-color);
            padding-bottom: 0.75rem;
            color: var(--accent-color);
            letter-spacing: 1px;
        }}
        .panel {{
            background-color: var(--panel-bg);
            border-radius: 0px;
            padding: 1rem;
            margin-bottom: 1rem;
            border-left: 2px solid var(--accent-color);
            position: relative;
        }}
        .url-container {{
            display: flex;
            align-items: stretch;
            position: relative;
            margin-bottom: 1.5rem;
        }}
        .panel-label {{
            font-size: 0.875rem;
            color: var(--muted-color);
            margin-bottom: 0.5rem;
            text-transform: uppercase;
            letter-spacing: 1px;
            display: block;
        }}
        .panel-value {{
            font-size: 1rem;
            word-break: break-all;
            font-family: monospace;
            background: rgba(0,0,0,0.2);
            padding: 6px 12px;
            border-radius: 0;
            border: 1px solid #333;
            flex: 1;
            overflow-x: auto;
            white-space: nowrap;
            border-right: none;
        }}
        .copy-button {{
            background-color: transparent;
            color: var(--muted-color);
            border: 1px solid #333;
            border-radius: 0;
            border-left: none;
            padding: 4px 8px;
            cursor: pointer;
            font-size: 0.75rem;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s;
            margin: 0;
        }}
        .copy-button:hover {{
            background-color: var(--hover-color);
            color: var(--accent-color);
        }}
        .status-row {{
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin: 1.5rem 0;
        }}
        .status-label {{
            font-size: 1.2rem;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 1px;
        }}
        .status-indicator {{
            font-weight: 600;
            padding: 0.35rem 1.25rem;
            border-radius: 0px;
            background-color: {};
            color: {};
            display: inline-block;
            letter-spacing: 1px;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
            gap: 1rem;
            margin-bottom: 1rem;
        }}
        .stat-panel {{
            background-color: var(--panel-bg);
            border-radius: 0px;
            padding: 1.25rem;
            border-left: 2px solid var(--accent-color);
        }}
        .stat-label {{
            font-size: 0.875rem;
            color: var(--muted-color);
            margin-bottom: 0.25rem;
            text-transform: uppercase;
            letter-spacing: 1px;
        }}
        .stat-value {{
            font-size: 1.75rem;
            font-weight: 600;
            color: var(--accent-color);
            font-family: monospace;
        }}
        .timestamp {{
            font-size: 0.875rem;
            color: var(--muted-color);
            text-align: right;
            margin-top: 1rem;
            font-family: monospace;
        }}
        .action-buttons {{
            display: flex;
            justify-content: center;
            gap: 1.5rem;
            margin-top: 2rem;
        }}
        .action-button {{
            background-color: transparent;
            color: var(--accent-color);
            border: 1px solid var(--accent-color);
            border-radius: 0px;
            padding: 0.6rem 1.25rem;
            font-size: 0.9rem;
            font-weight: 500;
            text-decoration: none;
            transition: all 0.2s;
            text-transform: uppercase;
            letter-spacing: 1px;
        }}
        .action-button:hover {{
            background-color: var(--accent-color);
            color: black;
            transform: translateY(-2px);
        }}
        .footer {{
            background-color: var(--footer-color);
            text-align: center;
            padding: 1.25rem;
            font-size: 0.875rem;
            color: var(--muted-color);
            margin-top: auto;
            border-top: 1px solid #333;
        }}
        .footer a {{
            color: var(--accent-color);
            text-decoration: none;
            transition: color 0.2s;
            font-weight: 500;
        }}
        .footer a:hover {{
            color: white;
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>Subgraph Monitor</h1>
            
            <div>
                <div class="panel-label">SUBGRAPH</div>
                <div class="url-container">
                    <div class="panel-value" id="subgraph-url">{}</div>
                    <button class="copy-button" onclick="copyToClipboard('subgraph-url')">
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                    </button>
                </div>
            </div>
            
            <div>
                <div class="panel-label">RPC ENDPOINT</div>
                <div class="url-container">
                    <div class="panel-value" id="rpc-url">{}</div>
                    <button class="copy-button" onclick="copyToClipboard('rpc-url')">
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                    </button>
                </div>
            </div>
            
            <div class="status-row">
                <div class="status-label">STATUS:</div>
                <div class="status-indicator">{}</div>
            </div>
            
            <div class="stats-grid">
                <div class="stat-panel">
                    <div class="stat-label">Synced Block</div>
                    <div class="stat-value">{}</div>
                </div>
                
                <div class="stat-panel">
                    <div class="stat-label">Chain Head</div>
                    <div class="stat-value">{}</div>
                </div>
                
                <div class="stat-panel">
                    <div class="stat-label">Blocks Behind</div>
                    <div class="stat-value">{}</div>
                </div>
            </div>
            
            <div class="timestamp">Last checked: {}</div>
            
            <div class="action-buttons">
                <a href="/health" class="action-button">JSON Health</a>
                <a href="/metrics" class="action-button">Prometheus Metrics</a>
            </div>
        </div>
    </div>
    
    <div class="footer">
        <span><a href="https://github.com/rome-blockchain-labs/subgraph-monitor">Fork source code here</a></span>
    </div>
    <script>
        function copyToClipboard(elementId) {{
            const element = document.getElementById(elementId);
            const text = element.textContent;
            
            // Create temporary textarea to copy from
            const textarea = document.createElement('textarea');
            textarea.value = text;
            textarea.style.position = 'fixed';  // Avoid scrolling to bottom
            document.body.appendChild(textarea);
            textarea.select();
            
            try {{
                // Execute copy command
                document.execCommand('copy');
                
            }} catch (err) {{
                console.error('Failed to copy text:', err);
            }}
            
            // Clean up
            document.body.removeChild(textarea);
        }}
    </script>
</body>
</html>"#,
        health_color,               // 1
        health_text_color,          // 2
        app_state.subgraph_url,     // 3
        app_state.rpc_url,          // 4
        health_text,                // 5
        status.synced_block_height, // 6
        status.chain_head_block_height, // 7
        status.blocks_behind,       // 8
        status.last_checked         // 9
    ))
}
