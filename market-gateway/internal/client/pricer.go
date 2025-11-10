package client

import (
	"context"
	"fmt"
	"time"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// PricerClient wraps the gRPC client for the FX pricing service
type PricerClient struct {
	conn   *grpc.ClientConn
	logger *zap.Logger
	addr   string
}

// NewPricerClient creates a new pricing service client
func NewPricerClient(addr string, logger *zap.Logger) (*PricerClient, error) {
	if logger == nil {
		var err error
		logger, err = zap.NewDevelopment()
		if err != nil {
			return nil, fmt.Errorf("failed to create logger: %w", err)
		}
	}

	return &PricerClient{
		addr:   addr,
		logger: logger,
	}, nil
}

// Connect establishes connection to the pricing service
func (c *PricerClient) Connect(ctx context.Context) error {
	c.logger.Info("connecting to pricing service", zap.String("address", c.addr))

	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	conn, err := grpc.DialContext(ctx, c.addr,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithBlock(),
	)
	if err != nil {
		return fmt.Errorf("failed to connect to pricing service at %s: %w", c.addr, err)
	}

	c.conn = conn
	c.logger.Info("successfully connected to pricing service")
	return nil
}

// Close closes the connection to the pricing service
func (c *PricerClient) Close() error {
	if c.conn != nil {
		c.logger.Info("closing connection to pricing service")
		return c.conn.Close()
	}
	return nil
}

// IsConnected returns true if the client is connected
func (c *PricerClient) IsConnected() bool {
	return c.conn != nil
}

// PriceRequest sends a price request to the service
// NOTE: This is a placeholder until protobuf types are generated
func (c *PricerClient) PriceRequest(ctx context.Context) error {
	if !c.IsConnected() {
		return fmt.Errorf("client not connected")
	}

	c.logger.Info("price request placeholder - awaiting protobuf generation")
	// TODO: Implement once proto files are generated:
	// client := pb.NewFXPricerClient(c.conn)
	// resp, err := client.Price(ctx, &pb.PriceRequest{...})
	return fmt.Errorf("not implemented: awaiting protobuf schema generation")
}

// UpdateMarket sends market data updates to the service
// NOTE: This is a placeholder until protobuf types are generated
func (c *PricerClient) UpdateMarket(ctx context.Context) error {
	if !c.IsConnected() {
		return fmt.Errorf("client not connected")
	}

	c.logger.Info("market update placeholder - awaiting protobuf generation")
	// TODO: Implement once proto files are generated:
	// client := pb.NewFXPricerClient(c.conn)
	// resp, err := client.UpdateMarket(ctx, &pb.MarketUpdate{...})
	return fmt.Errorf("not implemented: awaiting protobuf schema generation")
}

// HealthCheck queries the health status of the pricing service
// NOTE: This is a placeholder until protobuf types are generated
func (c *PricerClient) HealthCheck(ctx context.Context) error {
	if !c.IsConnected() {
		return fmt.Errorf("client not connected")
	}

	c.logger.Info("health check placeholder - awaiting protobuf generation")
	// TODO: Implement once proto files are generated:
	// client := pb.NewFXPricerClient(c.conn)
	// resp, err := client.Health(ctx, &pb.Empty{})
	return fmt.Errorf("not implemented: awaiting protobuf schema generation")
}
