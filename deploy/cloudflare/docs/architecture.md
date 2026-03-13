# MCP Gateway Architecture

Cloudflare Workers as an MCP Server Gateway to access internal APIs, resources, and services.

## 🏗️ High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              INTERNET / PUBLIC ZONE                             │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│   │    Claude    │    │   Cursor     │    │   VS Code    │    │  Custom App  │  │
│   │   Desktop    │    │     IDE      │    │   + Copilot  │    │  MCP Client  │  │
│   └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └──────┬───────┘  │
│          │                   │                   │                   │          │
│          └───────────────────┴─────────┬─────────┴───────────────────┘          │
│                                        │                                        │
│                                        ▼                                        │
│                        ┌───────────────────────────────┐                        │
│                        │      Cloudflare Edge CDN      │                        │
│                        │   (DDoS Protection, Caching)  │                        │
│                        └───────────────┬───────────────┘                        │
│                                        │                                        │
└────────────────────────────────────────┼────────────────────────────────────────┘
                                         │
┌────────────────────────────────────────┼────────────────────────────────────────┐
│                         CLOUDFLARE WORKERS ZONE                                 │
├────────────────────────────────────────┼────────────────────────────────────────┤
│                                        ▼                                        │
│              ┌──────────────────────────────────────────────────┐               │
│              │           MCP Gateway (Cloudflare Worker)        │               │
│              │  ┌─────────────────────────────────────────────┐ │               │
│              │  │              Request Router                 │ │               │
│              │  │   POST /mcp  │  GET /mcp  │  GET /health    │ │               │
│              │  └──────────────┼────────────┼─────────────────┘ │               │
│              │                 │            │                   │               │
│              │  ┌──────────────▼────────────▼─────────────────┐ │               │
│              │  │          Authentication Layer               │ │               │
│              │  │   API Key  │  Bearer Token  │  Basic Auth   │ │               │
│              │  └──────────────────────┬──────────────────────┘ │               │
│              │                         │                        │               │
│              │  ┌──────────────────────▼──────────────────────┐ │               │
│              │  │            MCP Protocol Handler             │ │               │
│              │  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐│ │               │
│              │  │  │ Tools  │ │Resources│ │Prompts │ │Complete│ │               │
│              │  │  │ Router │ │ Router │ │ Router │ │ Router ││ │               │
│              │  │  └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘│ │               │
│              │  └──────┼──────────┼──────────┼──────────┼───────┘               │
│              └─────────┼──────────┼──────────┼──────────┼───────                │
│                        │          │          │          │                           │
│   ┌────────────────────┼──────────┼──────────┼──────────┼────────────────────────┐  │
│   │                    ▼          ▼          ▼          ▼                        │  │
│   │  ┌─────────────────────────────────────────────────────────────────────┐ │  │
│   │  │                    Cloudflare Services Layer                        │ │  │
│   │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │ │  │
│   │  │  │    KV    │  │    R2    │  │   D1     │  │ Durable  │            │ │  │
│   │  │  │ Storage  │  │  Bucket  │  │ Database │  │ Objects  │            │ │  │
│   │  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │ │  │
│   │  └─────────────────────────────────────────────────────────────────────┘ │  │
│   │                               Bindings                                    │  │
│   └───────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
└───────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         │ Cloudflare Tunnel / Service Bindings
                                         │
┌────────────────────────────────────────┼────────────────────────────────────────┐
│                         PRIVATE NETWORK / INTERNAL ZONE                          │
├────────────────────────────────────────┼────────────────────────────────────────┤
│                                        ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────────┐   │
│   │                    Internal API Gateway / Load Balancer                  │   │
│   └─────────────────────────────────────────────────────────────────────────┘   │
│                    │              │              │              │                 │
│                    ▼              ▼              ▼              ▼                 │
│   ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────────┐ │
│   │   User Service   │ │  Product Service │ │   Order Service  │ │  Auth DB   │ │
│   │   (REST API)     │ │   (GraphQL)      │ │   (gRPC)         │ │ (Postgres) │ │
│   └──────────────────┘ └──────────────────┘ └──────────────────┘ └────────────┘ │
│                    │              │              │              │                 │
│   ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────────┐ │
│   │  Legacy System   │ │  File Storage    │ │  Message Queue   │ │  Cache     │ │
│   │  (SOAP/XML)      │ │  (S3/NFS)        │ │  (Kafka/RabbitMQ)│ │  (Redis)   │ │
│   └──────────────────┘ └──────────────────┘ └──────────────────┘ └────────────┘ │
│                                                                                   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

## 🔄 Request Flow

```
┌─────────┐     ┌───────────┐     ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  MCP    │     │ Cloudflare│     │ MCP Gateway │     │   Internal   │     │   Backend    │
│ Client  │     │   Edge    │     │   Worker    │     │   Service    │     │   Database   │
└────┬────┘     └─────┬─────┘     └──────┬──────┘     └──────┬───────┘     └──────┬───────┘
     │                │                  │                   │                    │
     │  1. MCP Request (JSON-RPC)        │                   │                    │
     │────────────────>│                 │                   │                    │
     │                │                  │                   │                    │
     │                │  2. Route to Worker                  │                    │
     │                │─────────────────>│                   │                    │
     │                │                  │                   │                    │
     │                │                  │ 3. Authenticate   │                    │
     │                │                  │ (API Key/Bearer)  │                    │
     │                │                  │                   │                    │
     │                │                  │ 4. Parse MCP Request                   │
     │                │                  │ (tools/call, resources/read, etc.)     │
     │                │                  │                   │                    │
     │                │                  │  5. Internal API Call                  │
     │                │                  │──────────────────>│                    │
     │                │                  │                   │                    │
     │                │                  │                   │  6. DB Query       │
     │                │                  │                   │───────────────────>│
     │                │                  │                   │                    │
     │                │                  │                   │  7. DB Response    │
     │                │                  │                   │<───────────────────│
     │                │                  │                   │                    │
     │                │                  │  8. API Response  │                    │
     │                │                  │<──────────────────│                    │
     │                │                  │                   │                    │
     │                │                  │ 9. Transform to MCP Response           │
     │                │                  │ (CallToolResult, ReadResourceResult)   │
     │                │                  │                   │                    │
     │                │ 10. JSON-RPC Response                │                    │
     │                │<─────────────────│                   │                    │
     │                │                  │                   │                    │
     │ 11. MCP Response                  │                   │                    │
     │<───────────────│                  │                   │                    │
     │                │                  │                   │                    │
```

## 🛡️ Security Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           SECURITY LAYERS                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  Layer 1: Edge Security (Cloudflare)                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  • DDoS Protection (automatic)                                          │ │
│  │  • WAF Rules (OWASP, custom)                                           │ │
│  │  • Rate Limiting (per IP, per API key)                                 │ │
│  │  • Bot Protection                                                       │ │
│  │  • SSL/TLS Termination                                                 │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                          │
│                                    ▼                                          │
│  Layer 2: Application Security (MCP Gateway)                                  │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  • Authentication (API Key, Bearer, Basic, OAuth2)                      │ │
│  │  • Authorization (Role-based access control)                            │ │
│  │  • Input Validation (JSON Schema)                                       │ │
│  │  • Request Sanitization                                                 │ │
│  │  • Audit Logging                                                        │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                          │
│                                    ▼                                          │
│  Layer 3: Network Security (Private Network)                                  │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  • Cloudflare Tunnel (Zero Trust)                                       │ │
│  │  • mTLS between services                                                │ │
│  │  • Network segmentation                                                 │ │
│  │  • Service mesh (optional)                                              │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                          │
│                                    ▼                                          │
│  Layer 4: Data Security (Backend)                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  • Encryption at rest                                                   │ │
│  │  • Encryption in transit                                                │ │
│  │  • Database access control                                              │ │
│  │  • Secrets management (Vault, Workers Secrets)                          │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 📦 Component Details

### MCP Gateway Worker

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MCP GATEWAY WORKER                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Entry Point (lib.rs)                            │ │
│  │  • HTTP Request handling                                               │ │
│  │  • CORS configuration                                                  │ │
│  │  • Route dispatching                                                   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                         │
│           ┌────────────────────────┼────────────────────────┐               │
│           ▼                        ▼                        ▼               │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│  │  Auth Module    │    │  MCP Server     │    │  Backend Client │         │
│  │  ─────────────  │    │  ───────────    │    │  ─────────────  │         │
│  │  • API Key      │    │  • Tools        │    │  • REST Client  │         │
│  │  • Bearer       │    │  • Resources    │    │  • GraphQL      │         │
│  │  • Basic        │    │  • Prompts      │    │  • gRPC         │         │
│  │  • OAuth2       │    │  • Completion   │    │  • WebSocket    │         │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘         │
│                                    │                                         │
│                                    ▼                                         │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                      Tool Handlers (tools/)                            │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                 │ │
│  │  │ user_lookup  │  │ create_order │  │ search_docs  │  ...            │ │
│  │  │ get_profile  │  │ update_item  │  │ run_query    │                 │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                 │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                         │
│                                    ▼                                         │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                   Resource Handlers (resources/)                       │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                 │ │
│  │  │ user://{id}  │  │ doc://{id}   │  │ config://app │  ...            │ │
│  │  │ order://{id} │  │ report://... │  │ schema://... │                 │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                 │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Internal Service Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    INTERNAL SERVICE INTEGRATION                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  MCP Tool                    Maps To                Internal Service         │
│  ────────                    ───────                ────────────────         │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ user_lookup      │  ──►  │ GET /api/users   │ ──►│  User Service    │   │
│  │ {user_id: str}   │       │    /{user_id}    │    │  (PostgreSQL)    │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ create_order     │  ──►  │ POST /api/orders │ ──►│  Order Service   │   │
│  │ {items: [...]}   │       │    {body}        │    │  (MongoDB)       │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ search_products  │  ──►  │ GraphQL Query    │ ──►│  Product Service │   │
│  │ {query: str}     │       │ products(q: $q)  │    │  (Elasticsearch) │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ run_analytics    │  ──►  │ gRPC Call        │ ──►│  Analytics       │   │
│  │ {report: str}    │       │ RunReport(req)   │    │  (ClickHouse)    │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  MCP Resource                Maps To                Internal Service         │
│  ────────────                ───────                ────────────────         │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ user://{id}      │  ──►  │ GET /api/users   │ ──►│  User Service    │   │
│  │                  │       │    /{id}         │    │                  │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ doc://{id}       │  ──►  │ S3 GetObject     │ ──►│  File Storage    │   │
│  │                  │       │ bucket/docs/{id} │    │  (R2/S3)         │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
│  ┌──────────────────┐       ┌──────────────────┐    ┌──────────────────┐   │
│  │ config://app     │  ──►  │ KV Get           │ ──►│  Config Store    │   │
│  │                  │       │ config:app       │    │  (Workers KV)    │   │
│  └──────────────────┘       └──────────────────┘    └──────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 🔗 Cloudflare Tunnel Setup

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CLOUDFLARE TUNNEL ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│                          Cloudflare Network                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                                                                       │  │
│  │   ┌─────────────────┐              ┌─────────────────┐               │  │
│  │   │  MCP Gateway    │◄────────────►│  Cloudflare     │               │  │
│  │   │  Worker         │   Binding    │  Tunnel         │               │  │
│  │   └─────────────────┘              └────────┬────────┘               │  │
│  │                                             │                        │  │
│  └─────────────────────────────────────────────┼────────────────────────┘  │
│                                                │                            │
│                                                │ Encrypted Tunnel           │
│                                                │ (No public IP needed)      │
│                                                │                            │
│  ┌─────────────────────────────────────────────┼────────────────────────┐  │
│  │                    Private Network          │                        │  │
│  │                                             ▼                        │  │
│  │   ┌─────────────────────────────────────────────────────────────┐   │  │
│  │   │                    cloudflared daemon                        │   │  │
│  │   │   (Runs on internal server, creates outbound tunnel)         │   │  │
│  │   └─────────────────────────────────────────────────────────────┘   │  │
│  │                    │              │              │                   │  │
│  │                    ▼              ▼              ▼                   │  │
│  │   ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐   │  │
│  │   │ localhost:8080   │ │ localhost:5432   │ │ localhost:6379   │   │  │
│  │   │ (REST API)       │ │ (PostgreSQL)     │ │ (Redis)          │   │  │
│  │   └──────────────────┘ └──────────────────┘ └──────────────────┘   │  │
│  │                                                                      │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘

# cloudflared config.yml example:
# tunnel: my-tunnel-id
# credentials-file: /etc/cloudflared/credentials.json
# ingress:
#   - hostname: api.internal
#     service: http://localhost:8080
#   - hostname: db.internal
#     service: tcp://localhost:5432
#   - service: http_status:404
```

## 📊 Monitoring & Observability

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       OBSERVABILITY ARCHITECTURE                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        MCP Gateway Worker                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                   │  │
│  │  │   Logging   │  │   Metrics   │  │   Tracing   │                   │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                   │  │
│  └─────────┼────────────────┼────────────────┼──────────────────────────┘  │
│            │                │                │                              │
│            ▼                ▼                ▼                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                   Cloudflare Analytics                               │   │
│  │  • Workers Analytics (requests, errors, latency)                     │   │
│  │  • Logpush (to S3, R2, Splunk, Datadog)                             │   │
│  │  • Real-time logs (wrangler tail)                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│            │                │                │                              │
│            ▼                ▼                ▼                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                     │
│  │   Grafana    │  │   Datadog    │  │   Splunk     │                     │
│  │  Dashboard   │  │    APM       │  │   SIEM       │                     │
│  └──────────────┘  └──────────────┘  └──────────────┘                     │
│                                                                              │
│  Key Metrics:                                                               │
│  ─────────────                                                              │
│  • Request rate (per tool, per resource)                                    │
│  • Error rate (4xx, 5xx by type)                                           │
│  • Latency (p50, p95, p99)                                                 │
│  • Auth failures (by method)                                                │
│  • Backend response times                                                   │
│  • Cache hit ratio                                                          │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 🚀 Deployment Options

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        DEPLOYMENT CONFIGURATIONS                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Option 1: Direct Backend Access (Simple)                                    │
│  ─────────────────────────────────────────                                   │
│                                                                              │
│    MCP Client ──► CF Worker ──► Public API (api.example.com)                │
│                                                                              │
│    Pros: Simple setup, no tunnel needed                                      │
│    Cons: Backend must be publicly accessible                                 │
│                                                                              │
│  ──────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  Option 2: Cloudflare Tunnel (Zero Trust)                                    │
│  ─────────────────────────────────────────                                   │
│                                                                              │
│    MCP Client ──► CF Worker ──► CF Tunnel ──► Private API (localhost:8080)  │
│                                                                              │
│    Pros: Backend stays private, zero trust model                             │
│    Cons: Requires cloudflared daemon                                         │
│                                                                              │
│  ──────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  Option 3: Service Bindings (Multi-Worker)                                   │
│  ─────────────────────────────────────────                                   │
│                                                                              │
│    MCP Client ──► MCP Gateway ──► Service Worker A                          │
│                        Worker ──► Service Worker B                          │
│                               ──► Service Worker C                          │
│                                                                              │
│    Pros: Isolation, independent scaling                                      │
│    Cons: More complex deployment                                             │
│                                                                              │
│  ──────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  Option 4: Hybrid (Production Recommended)                                   │
│  ─────────────────────────────────────────                                   │
│                                                                              │
│                     ┌──► CF KV (config)                                     │
│    MCP Client ──► CF│──► CF R2 (files)                                      │
│                Worker│──► CF D1 (metadata)                                   │
│                     └──► CF Tunnel ──► Internal APIs                        │
│                                                                              │
│    Pros: Best of all worlds, edge caching + private access                  │
│    Cons: More complex architecture                                          │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 📋 Implementation Checklist

```
□ Phase 1: Basic Setup
  □ Create Cloudflare Worker project
  □ Implement MCP protocol handler
  □ Add basic tools (health check, echo)
  □ Deploy to Cloudflare

□ Phase 2: Authentication
  □ Implement API Key auth
  □ Implement Bearer token auth
  □ Add rate limiting
  □ Set up Cloudflare secrets

□ Phase 3: Backend Integration
  □ Set up Cloudflare Tunnel
  □ Implement REST client
  □ Add tool handlers for internal APIs
  □ Add resource handlers

□ Phase 4: Cloudflare Services
  □ Integrate Workers KV (config, cache)
  □ Integrate R2 (file storage)
  □ Integrate D1 (metadata)
  □ Add Durable Objects (sessions)

□ Phase 5: Production Hardening
  □ Add WAF rules
  □ Set up Logpush
  □ Configure alerts
  □ Performance optimization
  □ Security audit
```

## 📚 References

- [Cloudflare Workers](https://developers.cloudflare.com/workers/)
- [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/)
- [MCP Specification](https://modelcontextprotocol.io/)
- [mcp-kit Documentation](https://github.com/KSD-CO/mcp-kit)
