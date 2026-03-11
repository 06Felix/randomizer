# Rust Random Data Streaming Service

A **high-performance schema-driven random data generation service** built in **Rust** that supports:

* **On-demand payload generation via HTTP REST**
* **Real-time streaming via WebSockets**

Clients provide a **JSON schema configuration** describing the structure of generated data. The server compiles the schema into a generator plan and produces random payloads efficiently.

This system is designed for **high throughput, low latency, and scalable data simulation workloads**.

---

# Features

* Schema-driven random payload generation
* REST API for single payload generation
* WebSocket streaming for continuous data generation
* Deterministic reproducibility using optional seeds
* Extensible datatype generator architecture
* High-performance async networking
* Stateless horizontally scalable design
* Trait-based generator system in Rust

---

# Use Cases

* Synthetic dataset generation
* Load testing APIs
* Data pipeline testing
* Event stream simulation
* IoT telemetry simulation
* Performance benchmarking

---

# Architecture Overview

The system follows a **schema-driven generator architecture**.

```
Client Schema
     │
     ▼
Schema Parser
     │
     ▼
Generator Compiler
     │
     ▼
Generator Engine
     │
     ├── REST API (single payload)
     └── WebSocket API (streaming)
```

### Key Design Principles

* Single source of generation logic
* Schema compiled into optimized generator plans
* Async event-driven networking
* Stateless service instances
* Per-connection generator isolation

---

# API Interfaces

## REST API

Generate a **single random payload**.

### Endpoint

```
POST /generate
```

### Flow

1. Client sends schema
2. Server parses configuration
3. Generator plan is compiled
4. Engine generates payload
5. JSON response returned

---

## WebSocket Streaming API

Stream random payloads continuously.

### Endpoint

```
WS /stream
```

### Flow

1. Client opens WebSocket connection
2. Client sends schema configuration
3. Server builds generator plan
4. Scheduler starts generation loop
5. Payloads are streamed at configured frequency

Streaming stops when the connection closes.

---

# Schema Configuration

Clients define payload structure using a JSON schema.

### Example Schema

```json
{
  "variables": {
    "user_id": {
      "datatype": "int",
      "min": 1,
      "max": 10000
    },
    "temperature": {
      "datatype": "float",
      "min": -10,
      "max": 50
    },
    "timestamp": {
      "datatype": "datetime"
    },
    "tags": {
      "datatype": "list",
      "values": ["A", "B", "C"]
    }
  },
  "frequency": "1s",
  "seed": 12345
}
```

---

# Supported Datatypes

| Datatype | Description                     |
| -------- | ------------------------------- |
| int      | Random integer in range         |
| float    | Random floating point number    |
| double   | Double precision floating point |
| datetime | Timestamp generator             |
| list     | Random value from provided list |
| boolean  | Random true/false               |
| string   | Random string generator         |
| uuid     | Random UUID                     |

---

# Generator Engine

The system uses a **trait-based polymorphic generator design**.

Each datatype implements a unified generator interface.

```rust
trait Generator {
    fn generate(&mut self) -> serde_json::Value;
}
```

This enables flexible extension of generator types.

---

# Random Number Generation

The system relies on high-quality pseudo-random number generators.

### Primary PRNG

**ChaCha20 RNG**

Advantages:

* Cryptographically secure
* High statistical quality
* Fast in software
* Resistant to prediction

### Secondary PRNG

**Xoshiro256++**

Advantages:

* Extremely fast
* High quality randomness
* Ideal for simulations

---

# Frequency Scheduler

Streaming payload generation uses a scheduler that parses human-readable durations.

### Supported Formats

```
1s
500ms
2s
5m
```

The scheduler runs an **async timer loop** to trigger payload generation.

---

# Concurrency Model

The service runs on an **async event-driven runtime**.

### Runtime

Tokio

### Execution Model

* One async task per WebSocket connection
* Independent generator instances per connection
* Non-blocking network IO
* Event loop scheduling

This architecture supports **thousands of concurrent streaming clients**.

---

# Project Structure

```
src
│
├── main.rs
│
├── api
│   ├── rest.rs
│   └── websocket.rs
│
├── schema
│   ├── config.rs
│   └── parser.rs
│
├── engine
│   ├── generator_trait.rs
│   └── engine.rs
│
├── generators
│   ├── int.rs
│   ├── float.rs
│   ├── datetime.rs
│   └── list.rs
│
├── scheduler
│   └── frequency.rs
│
└── utils
    ├── random.rs
    └── seed.rs
```

---

# Technology Stack

| Category          | Technology             |
| ----------------- | ---------------------- |
| Language          | Rust                   |
| Async Runtime     | Tokio                  |
| Web Framework     | Axum                   |
| WebSocket         | Axum WebSocket         |
| Serialization     | Serde                  |
| Random Generation | Rand                   |
| Datetime          | Chrono                 |
| Duration Parsing  | Humantime              |
| UUID              | uuid crate             |
| Logging           | Tracing                |
| Metrics           | Prometheus             |
| Testing           | Tokio Test + Criterion |

---

# Performance Optimizations

* Avoid unnecessary allocations
* Reuse generator instances
* Efficient PRNG implementations
* Non-blocking async networking
* Reduced JSON serialization overhead
* Per-thread RNG instances

Expected performance:

* **Millions of generated values per second**
* **Thousands of concurrent WebSocket connections**

---

# Scalability

The service is designed to be **stateless and horizontally scalable**.

### Deployment Model

```
Clients
   │
   ▼
Load Balancer
   │
   ▼
Multiple Service Instances
```

Each instance independently handles schema parsing and payload generation.

---

# Future Extensions

Planned enhancements:

* Nested object schemas
* Custom probability distributions
* Deterministic seed replay
* Synthetic dataset generation tools
* Kafka / streaming integrations
* Advanced schema validation

---

# License

MIT License
