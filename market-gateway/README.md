# Market Gateway

Go client for the FX Pricing Service - orchestrates pricing requests and manages market data for the Haskell FX pricer.

## Overview

The Market Gateway is a Go application that acts as the orchestrator between market data sources and the Haskell pricing service. It communicates via gRPC/Protocol Buffers to:

- Send price requests for FX contracts (spots, forwards, options)
- Push market data updates (spot rates, discount curves, volatility surfaces)
- Manage market data snapshots
- Process price responses

## Architecture

```
┌─────────────────┐                     ┌──────────────────┐
│  Market Gateway │   gRPC (Protobuf)   │ Haskell Pricing  │
│     (Go)        │────────────────────>│    Service       │
│                 │                     │                  │
│ - Market Data   │<────────────────────│ - Contract       │
│ - Orchestration │                     │   Algebra        │
│ - CLI Interface │                     │ - Black-Scholes  │
└─────────────────┘                     └──────────────────┘
```

## Project Structure

```
market-gateway/
├── cmd/
│   └── market-gateway/
│       └── main.go              # CLI application entry point
├── internal/
│   ├── client/
│   │   └── pricer.go            # gRPC client wrapper
│   ├── market/
│   │   └── manager.go           # Market data state management
│   ├── models/
│   │   └── contract.go          # Go contract builders
│   └── config/
│       └── config.go            # Configuration management
├── pkg/
│   └── proto/                   # Generated protobuf code (to be created)
├── examples/
│   └── simple_option.go         # Example: pricing a EUR/USD option
├── go.mod
├── go.sum
└── README.md
```

## Prerequisites

- Go 1.21 or higher
- Protocol Buffers compiler (protoc) - for future protobuf generation
- Access to the Haskell pricing service (running on localhost:50051 by default)

## Installation

```bash
# Clone the repository
git clone https://github.com/leonc/ficc-pricer.git
cd ficc-pricer/market-gateway

# Download dependencies
go mod download

# Build the application
go build -o bin/market-gateway ./cmd/market-gateway

# Run tests (when available)
go test ./...
```

## Dependencies

Current dependencies (see `go.mod`):

- `google.golang.org/grpc` - gRPC framework
- `google.golang.org/protobuf` - Protocol Buffers runtime
- `github.com/spf13/cobra` - CLI framework
- `github.com/spf13/viper` - Configuration management
- `go.uber.org/zap` - Structured logging

## Usage

### CLI Commands

The market gateway provides several commands:

```bash
# Price an FX contract
market-gateway price --contract option --strike 1.15 --maturity 2025-12-31

# Update market data
market-gateway update --spot EUR/USD=1.1050

# Check pricing service health
market-gateway health

# Run in daemon mode (future)
market-gateway serve --config config.yaml
```

### Example: Pricing a EUR/USD Call Option

See `examples/simple_option.go` for a complete example:

```bash
go run examples/simple_option.go
```

This example demonstrates:
1. Setting up market data (spot rates, curves, volatilities)
2. Building an FX option contract
3. Connecting to the pricing service
4. Requesting a price (placeholder until protobuf is generated)

### Configuration

Configuration can be provided via:
- Command-line flags
- Environment variables (prefix: `MG_`)
- Configuration file (`.market-gateway.yaml`)

Example configuration file:

```yaml
server:
  address: "localhost:50051"
  connect_timeout: 5
  request_timeout: 30
  enable_tls: false

logging:
  level: "info"
  format: "console"
  output_path: "stdout"

market:
  default_currency: "USD"
  default_volatility: 0.12
  default_rate: 0.05
  update_interval_ms: 1000
```

## Development Status

### ✅ Completed (Phase 1)

- [x] Go module initialization
- [x] Directory structure following Go standard layout
- [x] CLI skeleton with Cobra
- [x] Market data manager with thread-safe state
- [x] Contract models (Spot, Forward, EurOption, ZCB, Scale, Combine)
- [x] gRPC client wrapper (skeleton)
- [x] Configuration management with Viper
- [x] Example usage code

### 🚧 Next Steps (Requires Protobuf Schema)

The following features are blocked until the protobuf schema is defined:

1. **Create `pricer.proto`** - Define the complete protobuf schema based on ARCHITECTURE.md
   - Contract message types
   - Market data messages
   - RPC service definition

2. **Generate Go code**:
   ```bash
   protoc --go_out=pkg/proto --go-grpc_out=pkg/proto \
          --go_opt=paths=source_relative \
          --go-grpc_opt=paths=source_relative \
          pricer.proto
   ```

3. **Implement pricing calls** in `internal/client/pricer.go`
4. **Add contract-to-protobuf converters** to map Go models to protobuf messages
5. **Implement CLI command handlers** with actual gRPC calls
6. **Add integration tests** with the Haskell service

### 📋 Future Enhancements (Phase 2+)

- [ ] Pillar-based discount curves with interpolation
- [ ] Volatility grids (strike/maturity surface)
- [ ] Market data streaming
- [ ] Portfolio pricing (multiple contracts)
- [ ] Observable support for barrier options
- [ ] Performance monitoring and metrics
- [ ] Market data persistence

## Contract Types

The gateway supports the following contract types (mapped to Haskell GADTs):

| Type | Description | Example |
|------|-------------|---------|
| `Zero` | Zero-value contract | `Zero{}` |
| `Spot` | Spot FX trade | `NewSpot(USD, EUR)` |
| `Forward` | FX forward | `NewForward(date, 1.15, USD, EUR)` |
| `EurOption` | European option | `NewEurOption(Call, 1.15, date, USD, EUR)` |
| `ZCB` | Zero-coupon bond | `NewZCB(USD, date)` |
| `Scale` | Scaled contract | `NewScale(1000000, option)` |
| `Combine` | Combined contracts | `NewCombine(contract1, contract2)` |

## Market Data

The market manager supports:

- **Spot Rates**: FX pair spot rates (e.g., EUR/USD = 1.1050)
- **Discount Curves**: Currently flat rates (pillar curves coming in Phase 2)
- **Volatility Surfaces**: Currently flat volatility (grids coming in Phase 2)

All market data updates are thread-safe using read-write locks.

## Testing

```bash
# Run all tests
go test ./...

# Run tests with coverage
go test -cover ./...

# Run specific package tests
go test ./internal/market/
go test ./internal/models/
```

## Building for Production

```bash
# Build optimized binary
go build -ldflags="-s -w" -o bin/market-gateway ./cmd/market-gateway

# Cross-compile for Linux
GOOS=linux GOARCH=amd64 go build -o bin/market-gateway-linux ./cmd/market-gateway
```

## Troubleshooting

### Connection Refused

If you see `connection refused` when running commands:
- Ensure the Haskell pricing service is running
- Check the service is listening on `localhost:50051`
- Verify firewall settings

### Import Errors

If you see import errors after adding dependencies:
```bash
go mod tidy
go mod download
```

## Contributing

When adding new features:
1. Follow Go standard layout conventions
2. Add tests for new functionality
3. Update this README with new commands/features
4. Run `go fmt` and `go vet` before committing

## References

- [ARCHITECTURE.md](../ARCHITECTURE.md) - System architecture and protocol design
- [Go Project Layout](https://github.com/golang-standards/project-layout)
- [gRPC Go Quick Start](https://grpc.io/docs/languages/go/quickstart/)
- [Protocol Buffers Guide](https://developers.google.com/protocol-buffers/docs/overview)

## License

See the main project LICENSE file.
